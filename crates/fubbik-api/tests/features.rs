//! HTTP-level tests for the `features` domain — the 13 endpoints of
//! `packages/api/src/features/routes.ts`.
//!
//! Response-shape notes worth having in one place, all taken from Node:
//!
//! - `GET /api/features` is a **bare array** of list rows (`deltaCount`
//!   present, `userId` absent). `POST`/`PATCH`/`reorder` return the **full**
//!   feature row instead, `userId` included — three shapes, two structs.
//! - `GET /api/features/{id}` is `{ feature, spaces, deltas }`, not a
//!   flattened feature.
//! - `GET /api/features/active` is a bare array of **id strings**.
//! - `POST /api/features` is the only 201 in the domain.
//! - `DELETE`, `PUT /active`, `merge` and delta-delete all answer
//!   `{ message }` with a domain-specific string.
//!
//! Deep guards (user scoping inside the SQL) are proven at the repository
//! layer in `fubbik-db/tests/feature.rs`; this file proves the wiring, the
//! status codes and the shapes.

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

async fn seed_chunk(pool: &sqlx::PgPool, user_id: &str, title: &str) -> String {
    fubbik_db::repo::chunk::create(
        pool,
        user_id,
        fubbik_db::repo::chunk::NewChunk {
            title: title.into(),
            content: "base content".into(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id
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

async fn get(app: axum::Router, cookie: &str, path: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(path)
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn send(
    app: axum::Router,
    cookie: &str,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(method)
            .uri(path)
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn create_feature(
    app: axum::Router,
    cookie: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    let res = send(app, cookie, "POST", "/api/features", body).await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "POST /api/features answers 201, not 200"
    );
    json_body(res).await
}

// ---------------------------------------------------------------------------
// list / create
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_is_a_bare_array_of_list_rows_and_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-l@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-l@b.test", "Bob").await;

    create_feature(app.clone(), &alice, serde_json::json!({"name": "alices"})).await;
    create_feature(app.clone(), &bob, serde_json::json!({"name": "bobs"})).await;

    let body = json_body(get(app.clone(), &alice, "/api/features").await).await;
    let rows = body.as_array().expect("bare array, not an envelope");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], "alices");
    assert_eq!(
        rows[0]["deltaCount"], 0,
        "the list projection carries deltaCount"
    );
    assert!(
        rows[0].get("userId").is_none(),
        "the list projection must NOT leak userId — Node selects columns explicitly"
    );

    let bobs = json_body(get(app, &bob, "/api/features").await).await;
    assert_eq!(bobs.as_array().unwrap().len(), 1, "Bob's row is intact");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_returns_the_full_row_and_auto_assigns_priority(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-c@b.test", "Alice").await;

    let first = create_feature(
        app.clone(),
        &alice,
        serde_json::json!({"name": "first", "description": "d", "color": "#abc"}),
    )
    .await;
    assert_eq!(first["priority"], 1, "max(priority)=0 → first gets 1");
    assert_eq!(first["status"], "inactive", "column default");
    assert!(
        first["userId"].is_string(),
        "the create response IS the full row, userId included"
    );

    let second = create_feature(app.clone(), &alice, serde_json::json!({"name": "second"})).await;
    assert_eq!(second["priority"], 2);

    let explicit = create_feature(
        app,
        &alice,
        serde_json::json!({"name": "third", "priority": 42}),
    )
    .await;
    assert_eq!(explicit["priority"], 42, "an explicit priority is honoured");
}

/// `spaceId` is camelCase on the wire. A missing serde rename would make
/// this filter silently vanish and the endpoint return unscoped data while
/// looking healthy — so the assertion is that the filter actually *removes*
/// something.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_space_filter_is_wired_through_the_camel_case_query_key(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-sf@b.test", "Alice").await;
    let alice_id = user_id_for_email(&pool, "alice-sf@b.test").await;
    let s1 = seed_space(&pool, &alice_id, "s1").await;
    let s2 = seed_space(&pool, &alice_id, "s2").await;

    create_feature(
        app.clone(),
        &alice,
        serde_json::json!({"name": "in-s1", "spaceIds": [s1]}),
    )
    .await;
    create_feature(
        app.clone(),
        &alice,
        serde_json::json!({"name": "in-s2", "spaceIds": [s2]}),
    )
    .await;
    create_feature(app.clone(), &alice, serde_json::json!({"name": "global"})).await;

    let all = json_body(get(app.clone(), &alice, "/api/features").await).await;
    assert_eq!(all.as_array().unwrap().len(), 3);

    let filtered = json_body(get(app, &alice, &format!("/api/features?spaceId={s1}")).await).await;
    let names: Vec<&str> = filtered
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["in-s1", "global"],
        "the space filter must actually narrow the result, and must keep \
         features with no space association at all"
    );
}

// ---------------------------------------------------------------------------
// detail / update / delete
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detail_is_feature_spaces_deltas_and_404s_across_users(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-d@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-d@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-d@b.test").await;
    let space = seed_space(&pool, &alice_id, "s").await;

    let f = create_feature(
        app.clone(),
        &alice,
        serde_json::json!({"name": "f", "spaceIds": [space]}),
    )
    .await;
    let id = f["id"].as_str().unwrap();

    let body = json_body(get(app.clone(), &alice, &format!("/api/features/{id}")).await).await;
    assert_eq!(body["feature"]["id"], f["id"], "nested, not flattened");
    assert_eq!(body["spaces"].as_array().unwrap().len(), 1);
    assert_eq!(body["spaces"][0]["name"], "s");
    assert!(body["deltas"].as_array().unwrap().is_empty());

    let res = get(app.clone(), &bob, &format!("/api/features/{id}")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let res = get(app, &alice, "/api/features/nope").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patch_updates_clears_nulls_and_rejects_a_duplicate_name(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-p@b.test", "Alice").await;

    let taken = create_feature(app.clone(), &alice, serde_json::json!({"name": "taken"})).await;
    let f = create_feature(
        app.clone(),
        &alice,
        serde_json::json!({"name": "f", "description": "d", "color": "#abc"}),
    )
    .await;
    let id = f["id"].as_str().unwrap();

    let body = json_body(
        send(
            app.clone(),
            &alice,
            "PATCH",
            &format!("/api/features/{id}"),
            serde_json::json!({"name": "renamed", "status": "archived"}),
        )
        .await,
    )
    .await;
    assert_eq!(body["name"], "renamed");
    assert_eq!(body["status"], "archived");

    // Explicit nulls clear; omitted keys are untouched.
    let body = json_body(
        send(
            app.clone(),
            &alice,
            "PATCH",
            &format!("/api/features/{id}"),
            serde_json::json!({"description": null}),
        )
        .await,
    )
    .await;
    assert!(body["description"].is_null());
    assert_eq!(body["color"], "#abc", "an omitted key must not be cleared");

    // Renaming onto another feature's name is a 400 ValidationError, not a
    // 409 and not a unique-violation 500.
    let res = send(
        app.clone(),
        &alice,
        "PATCH",
        &format!("/api/features/{id}"),
        serde_json::json!({"name": taken["name"]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(
        json_body(res).await["message"]
            .as_str()
            .unwrap()
            .contains("already exists")
    );

    // Renaming a feature to the name it already has is legal.
    let res = send(
        app,
        &alice,
        "PATCH",
        &format!("/api/features/{id}"),
        serde_json::json!({"name": "renamed"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patch_space_ids_alone_is_accepted(pool: sqlx::PgPool) {
    // Node 500s on this: `spaceIds` is stripped before the row update, so
    // Drizzle receives `.set({})` and throws `No values to set`. This port
    // short-circuits to a re-select — a deliberate, flagged divergence.
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-si@b.test", "Alice").await;
    let alice_id = user_id_for_email(&pool, "alice-si@b.test").await;
    let space = seed_space(&pool, &alice_id, "s").await;

    let f = create_feature(app.clone(), &alice, serde_json::json!({"name": "f"})).await;
    let id = f["id"].as_str().unwrap();

    let res = send(
        app.clone(),
        &alice,
        "PATCH",
        &format!("/api/features/{id}"),
        serde_json::json!({"spaceIds": [space]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let detail = json_body(get(app, &alice, &format!("/api/features/{id}")).await).await;
    assert_eq!(detail["spaces"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_answers_a_message_and_404s_across_users(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-del@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-del@b.test", "Bob").await;
    let f = create_feature(app.clone(), &alice, serde_json::json!({"name": "f"})).await;
    let id = f["id"].as_str().unwrap();

    let res = send(
        app.clone(),
        &bob,
        "DELETE",
        &format!("/api/features/{id}"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = send(
        app.clone(),
        &alice,
        "DELETE",
        &format!("/api/features/{id}"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Deleted");

    // Unlike favorites, a second delete IS a 404 here.
    let res = send(
        app,
        &alice,
        "DELETE",
        &format!("/api/features/{id}"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// active features
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn active_features_round_trip_as_bare_id_strings(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-a@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-a@b.test", "Bob").await;

    let f1 = create_feature(app.clone(), &alice, serde_json::json!({"name": "f1"})).await;
    let f2 = create_feature(app.clone(), &alice, serde_json::json!({"name": "f2"})).await;
    let bobs = create_feature(app.clone(), &bob, serde_json::json!({"name": "bobs"})).await;

    let body = json_body(get(app.clone(), &alice, "/api/features/active").await).await;
    assert_eq!(body.as_array().unwrap().len(), 0);

    let res = send(
        app.clone(),
        &alice,
        "PUT",
        "/api/features/active",
        serde_json::json!({"featureIds": [f1["id"], f2["id"]]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Active features updated");

    let body = json_body(get(app.clone(), &alice, "/api/features/active").await).await;
    let mut got: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().expect("bare id strings, not objects"))
        .collect();
    got.sort();
    let mut want = vec![f1["id"].as_str().unwrap(), f2["id"].as_str().unwrap()];
    want.sort();
    assert_eq!(got, want);

    // Another user's feature id is a 400 and changes nothing.
    let res = send(
        app.clone(),
        &alice,
        "PUT",
        "/api/features/active",
        serde_json::json!({"featureIds": [f1["id"], bobs["id"]]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(
        json_body(res).await["message"]
            .as_str()
            .unwrap()
            .contains("Features not found")
    );
    let body = json_body(get(app.clone(), &alice, "/api/features/active").await).await;
    assert_eq!(
        body.as_array().unwrap().len(),
        2,
        "a rejected PUT must not have cleared the existing set"
    );

    // An empty array clears, with no validation at all.
    let res = send(
        app.clone(),
        &alice,
        "PUT",
        "/api/features/active",
        serde_json::json!({"featureIds": []}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(get(app.clone(), &alice, "/api/features/active").await).await;
    assert!(body.as_array().unwrap().is_empty());

    // `/features/active` must not be swallowed by `/features/{id}`.
    let res = get(app, &bob, "/api/features/active").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(json_body(res).await.is_array());
}

// ---------------------------------------------------------------------------
// reorder
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn reorder_returns_the_feature_and_shifts_the_rest_up(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-r@b.test", "Alice").await;

    let a = create_feature(
        app.clone(),
        &alice,
        serde_json::json!({"name": "a", "priority": 10}),
    )
    .await;
    let b = create_feature(
        app.clone(),
        &alice,
        serde_json::json!({"name": "b", "priority": 20}),
    )
    .await;

    // Move `b` to 10: everything at or above 10 shifts up (a: 10→11,
    // b: 20→21), then b is written to 10.
    let res = send(
        app.clone(),
        &alice,
        "POST",
        &format!("/api/features/{}/reorder", b["id"].as_str().unwrap()),
        serde_json::json!({"priority": 10}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["id"], b["id"], "reorder returns the feature row");
    assert_eq!(body["priority"], 10);

    let list = json_body(get(app.clone(), &alice, "/api/features").await).await;
    let pairs: Vec<(&str, i64)> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["name"].as_str().unwrap(), f["priority"].as_i64().unwrap()))
        .collect();
    assert_eq!(
        pairs,
        vec![("b", 10), ("a", 11)],
        "shift-up renumbering, no downward compaction"
    );

    // Reordering to the priority it already has is a no-op that shifts
    // nothing — the early return in the service.
    let res = send(
        app.clone(),
        &alice,
        "POST",
        &format!("/api/features/{}/reorder", a["id"].as_str().unwrap()),
        serde_json::json!({"priority": 11}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let list = json_body(get(app, &alice, "/api/features").await).await;
    let pairs: Vec<(&str, i64)> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["name"].as_str().unwrap(), f["priority"].as_i64().unwrap()))
        .collect();
    assert_eq!(pairs, vec![("b", 10), ("a", 11)]);
}

// ---------------------------------------------------------------------------
// deltas
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delta_crud_round_trip_and_shapes(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-dl@b.test", "Alice").await;
    let alice_id = user_id_for_email(&pool, "alice-dl@b.test").await;
    let chunk = seed_chunk(&pool, &alice_id, "Chunk Title").await;
    let f = create_feature(app.clone(), &alice, serde_json::json!({"name": "f"})).await;
    let fid = f["id"].as_str().unwrap();

    let res = send(
        app.clone(),
        &alice,
        "PUT",
        &format!("/api/chunks/{chunk}/deltas/{fid}"),
        serde_json::json!({"delta": {"content": "overlay content"}}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK, "PUT delta is 200, not 201");
    let created = json_body(res).await;
    assert_eq!(created["chunkId"], chunk.as_str());
    assert_eq!(created["featureId"], fid);
    assert_eq!(
        created["delta"],
        serde_json::json!({"content": "overlay content"}),
        "sparse: only the field that was sent"
    );

    // GET /chunks/{id}/deltas — carries the denormalised feature columns.
    let body =
        json_body(get(app.clone(), &alice, &format!("/api/chunks/{chunk}/deltas")).await).await;
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["featureName"], "f");
    assert_eq!(rows[0]["featurePriority"], 1);
    assert_eq!(rows[0]["featureStatus"], "inactive");

    // GET /features/{id}/deltas — carries the chunk title instead.
    let body =
        json_body(get(app.clone(), &alice, &format!("/api/features/{fid}/deltas")).await).await;
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["chunkTitle"], "Chunk Title");
    assert!(
        rows[0].get("featureName").is_none(),
        "the feature-side projection is a different shape"
    );

    let res = send(
        app.clone(),
        &alice,
        "DELETE",
        &format!("/api/chunks/{chunk}/deltas/{fid}"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Delta deleted");

    // Deleting again is a 404 ("Delta"), the feature still exists.
    let res = send(
        app,
        &alice,
        "DELETE",
        &format!("/api/chunks/{chunk}/deltas/{fid}"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(
        json_body(res).await["message"]
            .as_str()
            .unwrap()
            .contains("Delta")
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delta_validation_and_not_found_ordering(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-dv@b.test", "Alice").await;
    let alice_id = user_id_for_email(&pool, "alice-dv@b.test").await;
    let chunk = seed_chunk(&pool, &alice_id, "c").await;
    let f = create_feature(app.clone(), &alice, serde_json::json!({"name": "f"})).await;
    let fid = f["id"].as_str().unwrap();

    // Unknown fields → 400, naming the offenders.
    let res = send(
        app.clone(),
        &alice,
        "PUT",
        &format!("/api/chunks/{chunk}/deltas/{fid}"),
        serde_json::json!({"delta": {"unknownField": "bad", "tags": ["x"]}}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let msg = json_body(res).await["message"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        msg.contains("unknownField") && msg.contains("tags"),
        "{msg}"
    );

    // Empty delta → 400.
    let res = send(
        app.clone(),
        &alice,
        "PUT",
        &format!("/api/chunks/{chunk}/deltas/{fid}"),
        serde_json::json!({"delta": {}}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // All seven allowed fields → 200.
    let res = send(
        app.clone(),
        &alice,
        "PUT",
        &format!("/api/chunks/{chunk}/deltas/{fid}"),
        serde_json::json!({"delta": {
            "title": "T", "content": "C", "type": "document", "rationale": "R",
            "alternatives": ["A"], "consequences": "Q", "summary": "S"
        }}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    // Validation runs BEFORE the feature lookup: a bad delta aimed at a
    // nonexistent feature is a 400, not a 404.
    let res = send(
        app.clone(),
        &alice,
        "PUT",
        &format!("/api/chunks/{chunk}/deltas/nope"),
        serde_json::json!({"delta": {"bogus": 1}}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Feature is checked before chunk.
    let res = send(
        app.clone(),
        &alice,
        "PUT",
        "/api/chunks/no-such-chunk/deltas/no-such-feature",
        serde_json::json!({"delta": {"title": "T"}}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(
        json_body(res).await["message"]
            .as_str()
            .unwrap()
            .contains("Feature")
    );

    let res = send(
        app,
        &alice,
        "PUT",
        &format!("/api/chunks/no-such-chunk/deltas/{fid}"),
        serde_json::json!({"delta": {"title": "T"}}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(
        json_body(res).await["message"]
            .as_str()
            .unwrap()
            .contains("Chunk")
    );
}

/// Node's `GET /chunks/{id}/deltas` discards the session entirely, so any
/// signed-in caller can read any chunk's overlays. This port scopes it in
/// SQL — a deliberate, flagged divergence.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn chunk_deltas_do_not_leak_across_users(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-lk@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-lk@b.test", "Bob").await;
    let bob_id = user_id_for_email(&pool, "bob-lk@b.test").await;
    let bobs_chunk = seed_chunk(&pool, &bob_id, "bobs").await;
    let bobs_feature = create_feature(app.clone(), &bob, serde_json::json!({"name": "bobs"})).await;
    send(
        app.clone(),
        &bob,
        "PUT",
        &format!(
            "/api/chunks/{bobs_chunk}/deltas/{}",
            bobs_feature["id"].as_str().unwrap()
        ),
        serde_json::json!({"delta": {"content": "confidential"}}),
    )
    .await;

    let body = json_body(
        get(
            app.clone(),
            &alice,
            &format!("/api/chunks/{bobs_chunk}/deltas"),
        )
        .await,
    )
    .await;
    assert!(
        body.as_array().unwrap().is_empty(),
        "Alice must not see Bob's overlay content"
    );

    let body = json_body(get(app, &bob, &format!("/api/chunks/{bobs_chunk}/deltas")).await).await;
    assert_eq!(body.as_array().unwrap().len(), 1);
}

// ---------------------------------------------------------------------------
// merge
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn merge_applies_deltas_and_is_not_repeatable(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-m@b.test", "Alice").await;
    let alice_id = user_id_for_email(&pool, "alice-m@b.test").await;
    let chunk = seed_chunk(&pool, &alice_id, "original title").await;
    let f = create_feature(app.clone(), &alice, serde_json::json!({"name": "f"})).await;
    let fid = f["id"].as_str().unwrap();

    send(
        app.clone(),
        &alice,
        "PUT",
        &format!("/api/chunks/{chunk}/deltas/{fid}"),
        serde_json::json!({"delta": {"title": "merged title"}}),
    )
    .await;

    let res = send(
        app.clone(),
        &alice,
        "POST",
        &format!("/api/features/{fid}/merge"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Feature merged");

    // `GET /api/chunks/{id}` returns the enriched detail envelope, so the
    // merged row is under `chunk` — see `chunks::dto::ChunkDetail`.
    let body = json_body(get(app.clone(), &alice, &format!("/api/chunks/{chunk}")).await).await;
    assert_eq!(
        body["chunk"]["title"], "merged title",
        "delta folded into the base"
    );
    assert_eq!(
        body["_hasDeltas"], false,
        "merging deletes the deltas, so the merged chunk carries none"
    );

    let history =
        json_body(get(app.clone(), &alice, &format!("/api/chunks/{chunk}/history")).await).await;
    assert_eq!(history.as_array().unwrap().len(), 1);
    assert_eq!(
        history[0]["title"], "original title",
        "the snapshot is the pre-merge state"
    );

    let detail = json_body(get(app.clone(), &alice, &format!("/api/features/{fid}")).await).await;
    assert_eq!(detail["feature"]["status"], "merged");
    assert!(detail["deltas"].as_array().unwrap().is_empty());

    // Re-merging is a 400, not an idempotent 200.
    let res = send(
        app,
        &alice,
        "POST",
        &format!("/api/features/{fid}/merge"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(
        json_body(res).await["message"]
            .as_str()
            .unwrap()
            .contains("already merged")
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn merge_with_no_deltas_just_flips_the_status(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-me@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-me@b.test", "Bob").await;
    let f = create_feature(app.clone(), &alice, serde_json::json!({"name": "f"})).await;
    let fid = f["id"].as_str().unwrap();

    let res = send(
        app.clone(),
        &bob,
        "POST",
        &format!("/api/features/{fid}/merge"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND, "merge is user-scoped");

    let res = send(
        app.clone(),
        &alice,
        "POST",
        &format!("/api/features/{fid}/merge"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let detail = json_body(get(app, &alice, &format!("/api/features/{fid}")).await).await;
    assert_eq!(detail["feature"]["status"], "merged");
}

// ---------------------------------------------------------------------------
// auth
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn every_endpoint_requires_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    for (method, path) in [
        ("GET", "/api/features"),
        ("POST", "/api/features"),
        ("GET", "/api/features/active"),
        ("PUT", "/api/features/active"),
        ("GET", "/api/features/x"),
        ("PATCH", "/api/features/x"),
        ("DELETE", "/api/features/x"),
        ("POST", "/api/features/x/merge"),
        ("POST", "/api/features/x/reorder"),
        ("GET", "/api/features/x/deltas"),
        ("GET", "/api/chunks/x/deltas"),
        ("PUT", "/api/chunks/x/deltas/y"),
        ("DELETE", "/api/chunks/x/deltas/y"),
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path} must require a session"
        );
    }
}
