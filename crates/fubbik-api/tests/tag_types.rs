//! HTTP-level tests for the `tag_type` domain.
//!
//! Node's captured contract (`tests/fixtures/node-contract/tag-types-list.json`,
//! `_mutating.md`) is the source of truth here: `GET /api/tag-types` returns
//! a bare JSON array (not the `{chunks,...}` envelope chunks uses), create
//! is 201, update/delete are 200, and delete's body is `{ "message":
//! "Deleted" }`. Deletion never restricts or conflicts on referencing tags —
//! it's a database-level `ON DELETE SET NULL` foreign key, verified here by
//! inserting a raw `tag` row and checking it survives with `tag_type_id`
//! nulled out.

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
    }
}

/// Signs up a fresh user and returns the `name=value` session cookie pair
/// from the `set-cookie` response header, matching the pattern in
/// `tests/chunk_sub_resources.rs::signup`.
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

async fn create_tag_type(
    app: axum::Router,
    cookie: &str,
    name: &str,
    color: Option<&str>,
) -> axum::response::Response {
    let mut body = serde_json::json!({ "name": name });
    if let Some(color) = color {
        body["color"] = serde_json::Value::String(color.to_string());
    }
    app.oneshot(
        Request::post("/api/tag-types")
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn list_tag_types(app: axum::Router, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::get("/api/tag-types")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn patch_tag_type(
    app: axum::Router,
    cookie: &str,
    id: &str,
    name: &str,
) -> axum::response::Response {
    patch_tag_type_body(app, cookie, id, serde_json::json!({ "name": name })).await
}

async fn patch_tag_type_body(
    app: axum::Router,
    cookie: &str,
    id: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::patch(format!("/api/tag-types/{id}"))
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete_tag_type(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/tag-types/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_then_list_round_trip(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-create@b.test", "Alice").await;

    let res = create_tag_type(app.clone(), &cookie, "Topic", Some("#ff0000")).await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "create must return 201, matching Node's ctx.set.status = 201"
    );
    let created = json_body(res).await;
    assert_eq!(created["name"], "Topic");
    assert_eq!(created["color"], "#ff0000");
    assert_eq!(created["icon"], serde_json::Value::Null);
    let id = created["id"].as_str().unwrap().to_string();

    let res = list_tag_types(app.clone(), &cookie).await;
    assert_eq!(res.status(), StatusCode::OK);
    let listed = json_body(res).await;
    assert!(
        listed.is_array(),
        "GET /api/tag-types must return a bare array, not the {{chunks,...}} envelope"
    );
    let names: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Topic"]);
    assert_eq!(listed[0]["id"], id);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_without_color_falls_back_to_default(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-default@b.test", "Alice").await;

    let res = create_tag_type(app.clone(), &cookie, "Topic", None).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created = json_body(res).await;
    assert_eq!(created["color"], "#8b5cf6");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_patch_is_404_and_leaves_row_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-patch@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-patch@b.test", "Bob").await;

    let created = json_body(create_tag_type(app.clone(), &alice_cookie, "Topic", None).await).await;
    let id = created["id"].as_str().unwrap().to_string();

    let res = patch_tag_type(app.clone(), &bob_cookie, &id, "Hijacked").await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to update this tag type"
    );

    // A 404 that still mutated the row would be worse than a 200: prove
    // Alice's tag type is byte-for-byte unchanged after Bob's rejected PATCH.
    let listed = json_body(list_tag_types(app.clone(), &alice_cookie).await).await;
    let names: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["Topic"],
        "Alice's tag type must survive Bob's rejected PATCH"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_delete_is_404_and_leaves_row_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-delete@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-delete@b.test", "Bob").await;

    let created = json_body(create_tag_type(app.clone(), &alice_cookie, "Topic", None).await).await;
    let id = created["id"].as_str().unwrap().to_string();

    let res = delete_tag_type(app.clone(), &bob_cookie, &id).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to delete this tag type"
    );

    // Prove the rejected delete did not remove Alice's row.
    let listed = json_body(list_tag_types(app.clone(), &alice_cookie).await).await;
    let names: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["Topic"],
        "Alice's tag type must survive Bob's rejected DELETE"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_request_is_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .oneshot(Request::get("/api/tag-types").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_with_referencing_tags_nulls_tag_type_id_via_db_fk(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-fk@b.test", "Alice").await;

    let created = json_body(create_tag_type(app.clone(), &cookie, "Topic", None).await).await;
    let tag_type_id = created["id"].as_str().unwrap().to_string();
    let user_id: String =
        sqlx::query_scalar!(r#"SELECT user_id FROM tag_type WHERE id = $1"#, tag_type_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    // Insert a `tag` row directly (the tag repository/routes are a later
    // task in this slice) referencing the tag type, to exercise the FK.
    let tag_id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO tag (id, name, tag_type_id, user_id) VALUES ($1, $2, $3, $4)"#,
        tag_id,
        "rust",
        tag_type_id,
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let res = delete_tag_type(app.clone(), &cookie, &tag_type_id).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["message"], "Deleted");

    // The referencing tag survives, but with `tag_type_id` nulled out —
    // application code never touches the `tag` table; the database's own
    // `ON DELETE SET NULL` FK does this.
    let remaining_tag_type_id: Option<String> =
        sqlx::query_scalar!(r#"SELECT tag_type_id FROM tag WHERE id = $1"#, tag_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        remaining_tag_type_id, None,
        "tag.tag_type_id must be nulled out by the FK, not left dangling or blocked"
    );

    let tag_type_gone: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM tag_type WHERE id = $1"#,
        tag_type_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tag_type_gone, 0);
}

/// `icon` is tri-state, matching Node's `t.Optional(t.Union([t.String(),
/// t.Null()]))`. This exercises all three states in one flow — omitted
/// leaves it unchanged, explicit `null` clears it, a string sets it — and
/// reads the `icon` column back from the database directly rather than
/// trusting the response body, since the underlying bug this guards against
/// (`COALESCE($n, icon)` silently no-opping an explicit `null`) still
/// returned 200 with a response body that looked plausible.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_icon_tri_state(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-icon@b.test", "Alice").await;

    let created = json_body(create_tag_type(app.clone(), &cookie, "Topic", None).await).await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["icon"], serde_json::Value::Null);

    async fn read_icon(pool: &sqlx::PgPool, id: &str) -> Option<String> {
        sqlx::query_scalar!(r#"SELECT icon FROM tag_type WHERE id = $1"#, id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    // Set: a string value sets the column.
    let set = json_body(
        patch_tag_type_body(
            app.clone(),
            &cookie,
            &id,
            serde_json::json!({ "icon": "star" }),
        )
        .await,
    )
    .await;
    assert_eq!(set["icon"], "star");
    assert_eq!(read_icon(&pool, &id).await, Some("star".to_string()));

    // Absent: a patch that doesn't mention `icon` at all must leave it untouched.
    let untouched = json_body(
        patch_tag_type_body(
            app.clone(),
            &cookie,
            &id,
            serde_json::json!({ "name": "Topic2" }),
        )
        .await,
    )
    .await;
    assert_eq!(
        untouched["icon"], "star",
        "omitted icon key must leave the column untouched"
    );
    assert_eq!(read_icon(&pool, &id).await, Some("star".to_string()));

    // Clear: explicit `null` must clear the column to NULL — read the
    // column back from the database, not just the response body, since a
    // silent no-op would also return 200 with a plausible-looking body.
    let cleared = json_body(
        patch_tag_type_body(
            app.clone(),
            &cookie,
            &id,
            serde_json::json!({ "icon": null }),
        )
        .await,
    )
    .await;
    assert_eq!(cleared["icon"], serde_json::Value::Null);
    assert_eq!(
        read_icon(&pool, &id).await,
        None,
        "explicit null must clear icon to NULL in the database"
    );
    assert_eq!(
        cleared["name"], "Topic2",
        "clearing icon must not touch the name set by the previous PATCH"
    );
}
