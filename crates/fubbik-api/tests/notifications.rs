//! HTTP-level tests for the `notification` domain.
//!
//! Node's captured contract (`tests/fixtures/node-contract-2b/notifications-*.json`)
//! is the source of truth for response shape: `GET /api/notifications`
//! returns a bare JSON array, `GET /api/notifications/count` returns a bare
//! `{ "count": N }` object — neither is the `{chunks,total,limit,offset}`
//! envelope the chunks domain uses. `notification.type` is unconstrained
//! free text (no enum, no check constraint), so it round-trips arbitrary
//! strings.
//!
//! There is no `POST /api/notifications` create route in Node
//! (`packages/api/src/notifications/routes.ts` only wires the five routes
//! mirrored here), so every test seeds rows with a raw `INSERT` directly,
//! the same approach `tag_types.rs`'s FK test uses for the `tag` table.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
        better_auth_secret: "test-secret".into(),
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
        rate_limiter: Default::default(),
        background: Default::default(),
    }
}

/// Signs up a fresh user and returns the `name=value` session cookie pair
/// from the `set-cookie` response header, matching the pattern in
/// `tests/tag_types.rs::signup`.
async fn signup(app: axum::Router, email: &str, name: &str) -> String {
    let res = app
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"email":"{email}","password":"hunter22","name":"{name}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "signup must succeed");
    res.headers()
        .get("set-cookie")
        .expect("signup should set a session cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

/// Seeds a notification row directly via SQL for a given user id (there is
/// no HTTP create route to go through).
async fn seed(
    pool: &sqlx::PgPool,
    user_id: &str,
    notification_type: &str,
    title: &str,
    read: bool,
) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO notification (id, user_id, type, title, message, link_to, read)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        id,
        user_id,
        notification_type,
        title,
        "a message",
        Option::<&str>::None,
        read
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn user_id_for_email(pool: &sqlx::PgPool, email: &str) -> String {
    sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = $1"#, email)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn list_notifications(
    app: axum::Router,
    cookie: &str,
    query: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/notifications{query}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn unread_count(app: axum::Router, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::get("/api/notifications/count")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn mark_read(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::patch(format!("/api/notifications/{id}/read"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn mark_all_read(app: axum::Router, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/notifications/read-all")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete_notification(
    app: axum::Router,
    cookie: &str,
    id: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/notifications/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_returns_bare_array_and_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-list@b.test", "Bob").await;

    let alice_id = user_id_for_email(&pool, "alice-list@b.test").await;
    let bob_id = user_id_for_email(&pool, "bob-list@b.test").await;

    seed(
        &pool,
        &alice_id,
        "stale_chunks",
        "Alice's notification",
        false,
    )
    .await;
    seed(&pool, &bob_id, "stale_chunks", "Bob's notification", false).await;

    // When
    let res = list_notifications(app.clone(), &alice_cookie, "").await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(
        body.is_array(),
        "GET /api/notifications must return a bare array, not the {{chunks,...}} envelope"
    );
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Alice's notification"]);

    let res = list_notifications(app.clone(), &bob_cookie, "").await;
    let body = json_body(res).await;
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Bob's notification"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_unread_only_query_param_filters(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-unread@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-unread@b.test").await;

    seed(&pool, &user_id, "stale_chunks", "Unread one", false).await;
    seed(&pool, &user_id, "stale_chunks", "Already read", true).await;

    // When
    let body = json_body(list_notifications(app.clone(), &cookie, "").await).await;
    // Then
    assert_eq!(body.as_array().unwrap().len(), 2);

    let body = json_body(list_notifications(app.clone(), &cookie, "?unreadOnly=true").await).await;
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Unread one"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn count_returns_bare_count_object(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-count@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-count@b.test").await;

    // When
    let res = unread_count(app.clone(), &cookie).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(
        body,
        serde_json::json!({ "count": 0 }),
        "must match tests/fixtures/node-contract-2b/notifications-count.json shape"
    );

    seed(&pool, &user_id, "stale_chunks", "One", false).await;
    seed(&pool, &user_id, "stale_chunks", "Two", false).await;
    seed(&pool, &user_id, "stale_chunks", "Read", true).await;

    let body = json_body(unread_count(app.clone(), &cookie).await).await;
    assert_eq!(body, serde_json::json!({ "count": 2 }));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn mark_read_returns_the_notification_and_persists(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-mark@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-mark@b.test").await;

    let id = seed(&pool, &user_id, "stale_chunks", "Mark me", false).await;

    // When
    let res = mark_read(app.clone(), &cookie, &id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["id"], id);
    assert_eq!(body["read"], true);
    assert_eq!(body["title"], "Mark me");

    let read: bool = sqlx::query_scalar!("SELECT read FROM notification WHERE id = $1", id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(read);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_mark_read_is_404_and_leaves_victim_unread(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-crossmark@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-crossmark@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-crossmark@b.test").await;

    let id = seed(&pool, &alice_id, "stale_chunks", "Alice's", false).await;

    // When
    let res = mark_read(app.clone(), &bob_cookie, &id).await;
    // Then
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to mark this notification read"
    );

    let read: bool = sqlx::query_scalar!("SELECT read FROM notification WHERE id = $1", id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        !read,
        "Alice's notification must remain unread after Bob's rejected PATCH"
    );

    // Also verify from Alice's own view via the API, not just raw SQL.
    let body =
        json_body(list_notifications(app.clone(), &alice_cookie, "?unreadOnly=true").await).await;
    assert_eq!(body.as_array().unwrap().len(), 1);
}

/// Pins the exact wire message of a 404 in this domain to TitleCase,
/// matching Node's `"Notification not found"`
/// (`packages/api/src/notifications/service.ts:23,40`) and the house style
/// every other sibling domain in this slice uses. Covers both the
/// `mark_read` and `delete` 404 paths so a regression on either one goes
/// red, not just a status-code check.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn not_found_message_is_titlecase(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-404msg@b.test", "Alice").await;

    // When
    let res = mark_read(app.clone(), &cookie, "no-such-id").await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = json_body(res).await;
    assert_eq!(body["message"], "Notification not found");

    let res = delete_notification(app.clone(), &cookie, "no-such-id").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = json_body(res).await;
    assert_eq!(body["message"], "Notification not found");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn mark_all_read_marks_only_the_callers_notifications(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-markall@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-markall@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-markall@b.test").await;
    let bob_id = user_id_for_email(&pool, "bob-markall@b.test").await;

    seed(&pool, &alice_id, "stale_chunks", "Alice one", false).await;
    seed(&pool, &alice_id, "stale_chunks", "Alice two", false).await;
    let bob_notification_id = seed(&pool, &bob_id, "stale_chunks", "Bob's untouched", false).await;

    // When
    let res = mark_all_read(app.clone(), &alice_cookie).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "message": "All marked as read" }));

    let alice_count = json_body(unread_count(app.clone(), &alice_cookie).await).await;
    assert_eq!(alice_count, serde_json::json!({ "count": 0 }));

    // The bulk write must not have touched Bob's rows: this is the
    // destructive-cross-tenant-write scenario this task calls out
    // explicitly. Verify both via Bob's own count endpoint AND by reading
    // the row directly, so a bug that merely mis-scoped the count query
    // (but still mutated Bob's row) can't hide.
    let bob_count = json_body(unread_count(app.clone(), &bob_cookie).await).await;
    assert_eq!(
        bob_count,
        serde_json::json!({ "count": 1 }),
        "Bob's unread count must be untouched by Alice's mark_all_read"
    );
    let bob_read: bool = sqlx::query_scalar!(
        "SELECT read FROM notification WHERE id = $1",
        bob_notification_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !bob_read,
        "Bob's notification row must remain unread in the database"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_removes_the_notification(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-delete@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-delete@b.test").await;

    let id = seed(&pool, &user_id, "stale_chunks", "Delete me", false).await;

    // When
    let res = delete_notification(app.clone(), &cookie, &id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "message": "Deleted" }));

    let remaining: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM notification WHERE id = $1"#,
        id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_delete_is_404_and_leaves_victim_row_in_place(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-crossdel@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-crossdel@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-crossdel@b.test").await;

    let id = seed(&pool, &alice_id, "stale_chunks", "Alice's", false).await;

    // When
    let res = delete_notification(app.clone(), &bob_cookie, &id).await;
    // Then
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to delete this notification"
    );

    let remaining: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM notification WHERE id = $1"#,
        id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        remaining, 1,
        "Alice's notification must survive Bob's rejected DELETE"
    );

    let body = json_body(list_notifications(app.clone(), &alice_cookie, "").await).await;
    assert_eq!(body.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn notification_type_round_trips_arbitrary_strings(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-type@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-type@b.test").await;

    seed(
        &pool,
        &user_id,
        "some-totally-unconstrained-value",
        "Freeform type",
        false,
    )
    .await;

    // When
    let body = json_body(list_notifications(app.clone(), &cookie, "").await).await;
    // Then
    assert_eq!(
        body[0]["type"], "some-totally-unconstrained-value",
        "notification.type must round-trip any string — it is unconstrained free text in Node"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_requests_are_401(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));

    // When
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/notifications")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = app
        .oneshot(
            Request::get("/api/notifications/count")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
