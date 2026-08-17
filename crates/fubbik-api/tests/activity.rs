//! HTTP-level tests for the `activity` domain — the smallest in Phase 2b's
//! slice, a single route.
//!
//! `GET /api/activity` returns a bare JSON array with no `total` field
//! (`tests/fixtures/node-contract-2b/activity-list.json`,
//! `_questions.md` Q3/Q4) — not the `{chunks,total,limit,offset}` envelope
//! `chunks` uses. There is no mutating HTTP endpoint at all in Node
//! (`_mutating.md`'s "Activity" section: `createActivity` exists only as
//! an internal function other domains call directly), so every test here
//! seeds rows with a raw `INSERT`, the same approach `notifications.rs`
//! uses.
//!
//! The `spaceId` filter carries an ownership pre-check this port adds on
//! top of Node's bare equality filter (see
//! `fubbik_api::activity::service::list`'s doc comment for the full
//! divergence rationale) — a foreign `spaceId` 404s here where Node
//! returns 200 with `[]`. Proven load-bearing below by removing the
//! service-level pre-check and watching the API-level test still pass
//! (masked by the repo-level `EXISTS` guard), then removing that guard too
//! and watching the repo-level test fail instead — see
//! `crates/fubbik-db/tests/activity.rs` for that half, and the report for
//! the exact failure messages from both removals.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
        better_auth_secret: "test-secret".into(),
    }
}

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
    if body.is_empty() {
        return serde_json::Value::Null;
    }
    serde_json::from_slice(&body).unwrap()
}

async fn user_id_for_email(pool: &sqlx::PgPool, email: &str) -> String {
    sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = $1"#, email)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn seed_space(pool: &sqlx::PgPool, user_id: &str, name: &str) -> String {
    fubbik_db::repo::space::create(
        pool,
        user_id,
        fubbik_db::repo::space::NewSpace {
            name: name.into(),
            kind: "wiki".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id
}

#[allow(clippy::too_many_arguments)]
async fn seed_activity(
    pool: &sqlx::PgPool,
    user_id: &str,
    entity_type: &str,
    entity_id: &str,
    entity_title: Option<&str>,
    action: &str,
    space_id: Option<&str>,
) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO activity_log (id, user_id, entity_type, entity_id, entity_title, action, space_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        id,
        user_id,
        entity_type,
        entity_id,
        entity_title,
        action,
        space_id
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn list_activity(app: axum::Router, cookie: &str, query: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/activity{query}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_requires_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let res = app
        .oneshot(Request::get("/api/activity").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_returns_bare_array_and_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-list@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-list@b.test").await;
    let bob_id = user_id_for_email(&pool, "bob-list@b.test").await;

    seed_activity(
        &pool,
        &alice_id,
        "chunk",
        "c1",
        Some("Alice's chunk"),
        "created",
        None,
    )
    .await;
    seed_activity(
        &pool,
        &bob_id,
        "chunk",
        "c2",
        Some("Bob's chunk"),
        "created",
        None,
    )
    .await;

    let res = list_activity(app.clone(), &alice_cookie, "").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(
        body.is_array(),
        "GET /api/activity must return a bare array, not the {{chunks,...}} envelope"
    );
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["entityTitle"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Alice's chunk"]);
    assert!(
        !body.to_string().contains("total"),
        "response must not include a total field, matching activity-list.json"
    );

    let body = json_body(list_activity(app, &bob_cookie, "").await).await;
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["entityTitle"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Bob's chunk"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn entity_type_query_param_filters(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-et@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-et@b.test").await;

    seed_activity(&pool, &user_id, "chunk", "c1", None, "created", None).await;
    seed_activity(&pool, &user_id, "requirement", "r1", None, "created", None).await;

    let body = json_body(list_activity(app.clone(), &cookie, "").await).await;
    assert_eq!(body.as_array().unwrap().len(), 2);

    let body = json_body(list_activity(app, &cookie, "?entityType=requirement").await).await;
    let types: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["entityType"].as_str().unwrap())
        .collect();
    assert_eq!(types, vec!["requirement"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn limit_and_offset_query_params_paginate(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-page@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-page@b.test").await;

    for title in ["one", "two", "three"] {
        seed_activity(&pool, &user_id, "chunk", "c", Some(title), "created", None).await;
    }

    // Determined independently of the HTTP layer, in the same order the
    // service's own `ORDER BY created_at DESC, id ASC` produces — see
    // `fubbik_db::repo::activity::list`'s doc comment. Asserting against
    // this rather than a bare length means `limit`/`offset` being unwired
    // entirely (e.g. always returning the full unpaginated list) would fail
    // this test, not just happen to return the right count.
    let expected_order: Vec<String> = sqlx::query_scalar!(
        r#"SELECT entity_title AS "entity_title!" FROM activity_log
           WHERE user_id = $1 ORDER BY created_at DESC, id ASC"#,
        user_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_order.len(), 3);

    let body = json_body(list_activity(app.clone(), &cookie, "?limit=1").await).await;
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["entityTitle"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        vec![expected_order[0].as_str()],
        "limit=1 must return exactly the single most-recent row, not just any one row"
    );

    let body = json_body(list_activity(app.clone(), &cookie, "?limit=1&offset=1").await).await;
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["entityTitle"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        vec![expected_order[1].as_str()],
        "offset=1 must skip exactly the first row, not merely shrink the result count"
    );

    let body = json_body(list_activity(app, &cookie, "?limit=50&offset=3").await).await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

/// The escalated, brief-mandated divergence: a `spaceId` belonging to
/// another user must 404 rather than silently returning `[]` as Node does.
/// Bob's own activity (none, in this test) must be unaffected either way —
/// a status-only assertion would pass even if something leaked.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn space_id_filter_for_another_users_space_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_id = {
        signup(app.clone(), "alice-sp@b.test", "Alice").await;
        user_id_for_email(&pool, "alice-sp@b.test").await
    };
    let bob_cookie = signup(app.clone(), "bob-sp@b.test", "Bob").await;
    let alices_space = seed_space(&pool, &alice_id, "alices-space").await;

    seed_activity(
        &pool,
        &alice_id,
        "chunk",
        "c1",
        Some("Alice's row"),
        "created",
        Some(&alices_space),
    )
    .await;

    let res = list_activity(app, &bob_cookie, &format!("?spaceId={alices_space}")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn space_id_filter_for_nonexistent_space_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-sp-none@b.test", "Alice").await;

    let res = list_activity(app, &cookie, "?spaceId=no-such-space").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn space_id_filter_for_the_callers_own_space_narrows_results(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-sp-own@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-sp-own@b.test").await;
    let alices_space = seed_space(&pool, &user_id, "alices-space").await;

    seed_activity(
        &pool,
        &user_id,
        "chunk",
        "c1",
        Some("in space"),
        "created",
        Some(&alices_space),
    )
    .await;
    seed_activity(
        &pool,
        &user_id,
        "chunk",
        "c2",
        Some("global"),
        "created",
        None,
    )
    .await;

    let body =
        json_body(list_activity(app, &cookie, &format!("?spaceId={alices_space}")).await).await;
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["entityTitle"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["in space"]);
}

/// `action`/`entityType` round-trip arbitrary strings — no enum, no check
/// constraint in Node, see `fubbik_db::repo::activity`'s module doc.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn action_and_entity_type_round_trip_free_text(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-free@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-free@b.test").await;

    seed_activity(
        &pool,
        &user_id,
        "totally-made-up-entity",
        "x1",
        None,
        "totally-made-up-action",
        None,
    )
    .await;

    let body = json_body(list_activity(app, &cookie, "").await).await;
    let row = &body.as_array().unwrap()[0];
    assert_eq!(row["entityType"], "totally-made-up-entity");
    assert_eq!(row["action"], "totally-made-up-action");
}
