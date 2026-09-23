//! HTTP-level tests for the two vocabulary catalogs
//! (`GET/POST /api/chunk-types`, `PATCH/DELETE /api/chunk-types/{id}` and
//! the same four for `/api/connection-relations`).
//!
//! The cross-user SQL ownership guards on `list`/`update`/`delete` are
//! proven load-bearing at the repo level in `fubbik-db/tests/{chunk_type,
//! connection_relation}.rs`, which observe them directly. The cross-user
//! tests here do additionally catch a removed `update`/`delete` guard
//! (verified: the response flips 404 -> 200), because the service's
//! unscoped `find_by_id` pre-check does not short-circuit a cross-user
//! attempt — the row genuinely exists, so the request reaches SQL. That is
//! a happy accident of this domain's shape, not something to rely on; the
//! repo tests remain the authoritative proof.
//!
//! What THIS file proves that the repo tests cannot:
//!   * the service-layer `builtIn` rejections
//!     (`packages/api/src/vocabularies/service.ts:61-63,78-80,124-126,141-143`)
//!     surface as **400** with the exact copied message, not 404 or 500;
//!   * the slug and duplicate-id validations are 400s;
//!   * `spaceId` is read off the wire under its camelCase name (a missing
//!     `rename_all` would make serde silently ignore it);
//!   * the JSON response shapes, including `{ "message": "Deleted" }`.

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

async fn post(
    app: axum::Router,
    cookie: &str,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post(path)
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn patch(
    app: axum::Router,
    cookie: &str,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::patch(path)
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete(app: axum::Router, cookie: &str, path: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(path)
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

fn message(body: &serde_json::Value) -> &str {
    body["message"].as_str().unwrap_or_default()
}

// --- chunk types ------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_chunk_types_returns_bare_array_of_builtins_and_own_only(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "alice-ct-list@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-ct-list@b.test", "Bob").await;

    post(
        app.clone(),
        &alice,
        "/api/chunk-types",
        serde_json::json!({ "id": "alices", "label": "Alice's" }),
    )
    .await;
    post(
        app.clone(),
        &bob,
        "/api/chunk-types",
        serde_json::json!({ "id": "bobs", "label": "Bob's" }),
    )
    .await;

    // When
    let res = get(app.clone(), &alice, "/api/chunk-types").await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(body.is_array(), "must return a bare array");
    let ids: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"alices"));
    assert!(ids.contains(&"convention"), "seeded built-ins are included");
    assert!(!ids.contains(&"bobs"), "must not leak another user's row");

    // Ordering is Node's own `display_order ASC, id ASC`; the seeded
    // built-ins run 10..70 so a custom row defaulting to 500 sorts last.
    let orders: Vec<i64> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["displayOrder"].as_i64().unwrap())
        .collect();
    let mut sorted = orders.clone();
    sorted.sort_unstable();
    assert_eq!(orders, sorted);
}

/// `spaceId` must be read off the wire under its **camelCase** name. Serde
/// silently ignores an unknown key rather than rejecting it, so a missing
/// `#[serde(rename_all = "camelCase")]` on `ChunkTypesQuery` would leave
/// `space_id` permanently `None` and this endpoint would look perfectly
/// healthy while quietly ignoring the parameter. The space-scoped row below
/// only appears when the parameter genuinely reaches the SQL.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_chunk_types_reads_space_id_under_its_camel_case_wire_name(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-ct-space@b.test", "Alice").await;
    let uid: String = sqlx::query_scalar!("SELECT id FROM \"user\" LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let space_id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO space (id, name, kind, user_id) VALUES ($1, 'S', 'code', $2)"#,
        space_id,
        uid
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!(
        r#"INSERT INTO chunk_type (id, label, space_id) VALUES ('spaced', 'Spaced', $1)"#,
        space_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let without = json_body(get(app.clone(), &cookie, "/api/chunk-types").await).await;
    // When
    let has = |b: &serde_json::Value| {
        b.as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == serde_json::json!("spaced"))
    };
    // Then
    assert!(!has(&without), "absent spaceId must not surface the row");

    let with = json_body(
        get(
            app.clone(),
            &cookie,
            &format!("/api/chunk-types?spaceId={space_id}"),
        )
        .await,
    )
    .await;
    assert!(
        has(&with),
        "spaceId must reach the SQL — if this fails, the camelCase rename \
         on ChunkTypesQuery is missing and serde is dropping the parameter"
    );

    // `?spaceId=` (empty) is `undefined` to Node, i.e. identical to absent.
    let empty = json_body(get(app.clone(), &cookie, "/api/chunk-types?spaceId=").await).await;
    assert!(!has(&empty), "an empty spaceId must behave as if omitted");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_chunk_type_returns_201_with_nodes_defaults(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-ct-create@b.test", "Alice").await;

    // When
    let res = post(
        app.clone(),
        &cookie,
        "/api/chunk-types",
        serde_json::json!({ "id": "runbook", "label": "Runbook" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["id"], "runbook");
    assert_eq!(body["label"], "Runbook");
    assert_eq!(body["color"], "#8b5cf6");
    assert_eq!(body["examples"], serde_json::json!([]));
    assert_eq!(body["displayOrder"], 500);
    assert_eq!(body["builtIn"], false);
    assert_eq!(body["spaceId"], serde_json::Value::Null);
    assert!(body["userId"].is_string());
    assert!(body["createdAt"].is_string());
    assert!(body["updatedAt"].is_string());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_chunk_type_round_trips_every_optional_field(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-ct-full@b.test", "Alice").await;

    // When
    let res = post(
        app.clone(),
        &cookie,
        "/api/chunk-types",
        serde_json::json!({
            "id": "runbook-2",
            "label": "Runbook",
            "description": "Ops steps",
            "icon": "BookOpen",
            "color": "#123456",
            "examples": ["Restart", "Rollback"],
            "displayOrder": 7
        }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["description"], "Ops steps");
    assert_eq!(body["icon"], "BookOpen");
    assert_eq!(body["color"], "#123456");
    assert_eq!(body["examples"], serde_json::json!(["Restart", "Rollback"]));
    assert_eq!(body["displayOrder"], 7);
}

/// Node's `SLUG_RE` check is a **service**-layer `ValidationError` (400),
/// not just an Elysia schema cap (`packages/api/src/vocabularies/service.ts:42-46`).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_chunk_type_rejects_a_non_slug_id_with_400(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-ct-slug@b.test", "Alice").await;

    for bad in ["Upper", "_leading", "has space", "", &"a".repeat(42)] {
        // When
        let res = post(
            app.clone(),
            &cookie,
            "/api/chunk-types",
            serde_json::json!({ "id": bad, "label": "X" }),
        )
        .await;
        // Then
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "id {bad:?}");
        let body = json_body(res).await;
        assert!(
            message(&body).contains("id must be a lowercase slug"),
            "id {bad:?} -> {}",
            message(&body)
        );
    }
}

/// The duplicate check is unscoped (the slug is the table's primary key),
/// so a built-in's id is taken for everyone.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_chunk_type_rejects_a_duplicate_id_with_400(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "alice-ct-dup@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-ct-dup@b.test", "Bob").await;

    // When
    // Against a seeded built-in.
    let res = post(
        app.clone(),
        &alice,
        "/api/chunk-types",
        serde_json::json!({ "id": "convention", "label": "Mine" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(
        message(&json_body(res).await).contains(r#"chunk type "convention" already exists"#),
        "message must name the chunk-type catalog, not the relation one"
    );

    // And against another user's custom row.
    post(
        app.clone(),
        &alice,
        "/api/chunk-types",
        serde_json::json!({ "id": "shared", "label": "Alice's" }),
    )
    .await;
    let res = post(
        app.clone(),
        &bob,
        "/api/chunk-types",
        serde_json::json!({ "id": "shared", "label": "Bob's" }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "slugs are global, not per-user"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_chunk_type_applies_patch_and_leaves_omitted_fields_alone(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-ct-patch@b.test", "Alice").await;
    post(
        app.clone(),
        &cookie,
        "/api/chunk-types",
        serde_json::json!({ "id": "mine", "label": "Mine", "description": "d", "icon": "I" }),
    )
    .await;

    // When
    let res = patch(
        app.clone(),
        &cookie,
        "/api/chunk-types/mine",
        serde_json::json!({ "label": "Renamed", "description": null }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["label"], "Renamed");
    assert_eq!(
        body["description"],
        serde_json::Value::Null,
        "explicit null must clear"
    );
    assert_eq!(body["icon"], "I", "omitted field must stay untouched");
}

/// Node's built-in rejection (`service.ts:61-63`). Without the
/// service-layer `built_in` check the row would still be unmutable (its
/// `user_id` is `NULL`), but the response would be 404 — so this assertion
/// on 400 is what pins the check.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_chunk_type_rejects_built_in_with_400(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-ct-bi@b.test", "Alice").await;

    // When
    let res = patch(
        app.clone(),
        &cookie,
        "/api/chunk-types/convention",
        serde_json::json!({ "label": "hijacked" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(
        message(&json_body(res).await).contains("builtin chunk types cannot be edited"),
        "message is copied verbatim from Node"
    );

    let still: String = sqlx::query_scalar!("SELECT label FROM chunk_type WHERE id = 'convention'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(still, "Convention", "built-in row must be unmutated");
}

/// Node's built-in rejection on delete (`service.ts:78-80`) — the status
/// code matters as much as the refusal, since a plain 404 would be
/// indistinguishable from "no such id".
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_chunk_type_rejects_built_in_with_400(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-ct-bid@b.test", "Alice").await;

    // When
    let res = delete(app.clone(), &cookie, "/api/chunk-types/convention").await;
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(message(&json_body(res).await).contains("builtin chunk types cannot be deleted"));

    let count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM chunk_type WHERE id = 'convention'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "built-in row must not be deleted");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn chunk_type_cross_user_mutations_are_404_and_leave_the_row_alone(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-ct-x@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-ct-x@b.test", "Bob").await;
    post(
        app.clone(),
        &alice,
        "/api/chunk-types",
        serde_json::json!({ "id": "alices", "label": "Alice's" }),
    )
    .await;

    // When
    let res = patch(
        app.clone(),
        &bob,
        "/api/chunk-types/alices",
        serde_json::json!({ "label": "hijacked" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = delete(app.clone(), &bob, "/api/chunk-types/alices").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let still: String = sqlx::query_scalar!("SELECT label FROM chunk_type WHERE id = 'alices'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(still, "Alice's");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unknown_chunk_type_id_is_404(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    // When
    let cookie = signup(app.clone(), "alice-ct-404@b.test", "Alice").await;

    // Then
    assert_eq!(
        patch(
            app.clone(),
            &cookie,
            "/api/chunk-types/nope",
            serde_json::json!({ "label": "x" })
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        delete(app.clone(), &cookie, "/api/chunk-types/nope")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_chunk_type_returns_message_deleted_and_removes_row(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-ct-del@b.test", "Alice").await;
    post(
        app.clone(),
        &cookie,
        "/api/chunk-types",
        serde_json::json!({ "id": "disposable", "label": "D" }),
    )
    .await;

    // When
    let res = delete(app.clone(), &cookie, "/api/chunk-types/disposable").await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({ "message": "Deleted" })
    );

    let count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM chunk_type WHERE id = 'disposable'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

// --- connection relations ---------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_connection_relations_returns_bare_array_of_builtins_and_own_only(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "alice-cr-list@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-cr-list@b.test", "Bob").await;

    post(
        app.clone(),
        &alice,
        "/api/connection-relations",
        serde_json::json!({ "id": "alices", "label": "Alice's" }),
    )
    .await;
    post(
        app.clone(),
        &bob,
        "/api/connection-relations",
        serde_json::json!({ "id": "bobs", "label": "Bob's" }),
    )
    .await;

    // When
    let res = get(app.clone(), &alice, "/api/connection-relations").await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(body.is_array());
    let ids: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"alices"));
    assert!(ids.contains(&"depends_on"), "seeded built-ins are included");
    assert!(!ids.contains(&"bobs"));

    // Built-in `depends_on` carries the full wire shape, camelCase keys and
    // all — including the self-referential `inverseOfId` the seed sets.
    let depends_on = body
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == serde_json::json!("depends_on"))
        .unwrap();
    assert_eq!(depends_on["label"], "Depends on");
    assert_eq!(depends_on["arrowStyle"], "solid");
    assert_eq!(depends_on["direction"], "forward");
    assert_eq!(depends_on["inverseOfId"], "required_by");
    assert_eq!(depends_on["builtIn"], true);
    assert_eq!(depends_on["userId"], serde_json::Value::Null);
}

/// Same camelCase-wire-name proof as the chunk-type version — a missing
/// `rename_all` on `ConnectionRelationsQuery` would make serde drop the
/// parameter silently.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_connection_relations_reads_space_id_under_its_camel_case_wire_name(
    pool: sqlx::PgPool,
) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-cr-space@b.test", "Alice").await;
    let uid: String = sqlx::query_scalar!("SELECT id FROM \"user\" LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let space_id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO space (id, name, kind, user_id) VALUES ($1, 'S', 'code', $2)"#,
        space_id,
        uid
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!(
        r#"INSERT INTO connection_relation (id, label, space_id) VALUES ('spaced', 'Spaced', $1)"#,
        space_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let has = |b: &serde_json::Value| {
        b.as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == serde_json::json!("spaced"))
    };

    // When
    let without = json_body(get(app.clone(), &cookie, "/api/connection-relations").await).await;
    // Then
    assert!(!has(&without));

    let with = json_body(
        get(
            app.clone(),
            &cookie,
            &format!("/api/connection-relations?spaceId={space_id}"),
        )
        .await,
    )
    .await;
    assert!(has(&with), "spaceId must reach the SQL");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_connection_relation_returns_201_with_nodes_defaults(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cr-create@b.test", "Alice").await;

    // When
    let res = post(
        app.clone(),
        &cookie,
        "/api/connection-relations",
        serde_json::json!({ "id": "mirrors", "label": "Mirrors" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["id"], "mirrors");
    assert_eq!(body["arrowStyle"], "solid");
    assert_eq!(body["direction"], "forward");
    assert_eq!(body["color"], "#64748b");
    assert_eq!(body["displayOrder"], 500);
    assert_eq!(body["builtIn"], false);
    assert_eq!(body["inverseOfId"], serde_json::Value::Null);
    assert_eq!(body["spaceId"], serde_json::Value::Null);
}

/// `arrowStyle`/`direction` are genuine `t.Union` literals in Node, so an
/// out-of-set value is a 400 — unlike the many "constrained-looking but
/// actually free text" fields elsewhere in this port.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_connection_relation_accepts_every_literal_and_rejects_others(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cr-enum@b.test", "Alice").await;

    for (i, style) in ["solid", "dashed", "dotted"].iter().enumerate() {
        for (j, dir) in ["forward", "bidirectional"].iter().enumerate() {
            // When
            let res = post(
                app.clone(),
                &cookie,
                "/api/connection-relations",
                serde_json::json!({
                    "id": format!("r{i}{j}"),
                    "label": "R",
                    "arrowStyle": style,
                    "direction": dir
                }),
            )
            .await;
            // Then
            assert_eq!(res.status(), StatusCode::CREATED, "{style}/{dir}");
            let body = json_body(res).await;
            assert_eq!(body["arrowStyle"], *style);
            assert_eq!(body["direction"], *dir);
        }
    }

    let res = post(
        app.clone(),
        &cookie,
        "/api/connection-relations",
        serde_json::json!({ "id": "bad", "label": "R", "arrowStyle": "squiggly" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_connection_relation_rejects_non_slug_and_duplicate_ids_with_400(
    pool: sqlx::PgPool,
) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cr-val@b.test", "Alice").await;

    // When
    let res = post(
        app.clone(),
        &cookie,
        "/api/connection-relations",
        serde_json::json!({ "id": "Not A Slug", "label": "R" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(message(&json_body(res).await).contains("id must be a lowercase slug"));

    let res = post(
        app.clone(),
        &cookie,
        "/api/connection-relations",
        serde_json::json!({ "id": "depends_on", "label": "R" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(
        message(&json_body(res).await).contains(r#"relation "depends_on" already exists"#),
        "this catalog's duplicate message says `relation`, not `chunk type`"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_connection_relation_applies_patch_including_tri_state_inverse_of_id(
    pool: sqlx::PgPool,
) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cr-patch@b.test", "Alice").await;
    post(
        app.clone(),
        &cookie,
        "/api/connection-relations",
        serde_json::json!({
            "id": "mine", "label": "Mine",
            "description": "d", "inverseOfId": "related_to"
        }),
    )
    .await;

    // When
    let res = patch(
        app.clone(),
        &cookie,
        "/api/connection-relations/mine",
        serde_json::json!({ "label": "Renamed", "arrowStyle": "dotted" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["label"], "Renamed");
    assert_eq!(body["arrowStyle"], "dotted");
    assert_eq!(body["description"], "d", "omitted stays untouched");
    assert_eq!(body["inverseOfId"], "related_to", "omitted stays untouched");

    let res = patch(
        app.clone(),
        &cookie,
        "/api/connection-relations/mine",
        serde_json::json!({ "inverseOfId": null, "description": null }),
    )
    .await;
    let body = json_body(res).await;
    assert_eq!(body["inverseOfId"], serde_json::Value::Null);
    assert_eq!(body["description"], serde_json::Value::Null);
}

/// Node's built-in rejections for this catalog use a *different* message
/// ("builtin relations …", not "builtin chunk types …") — checked
/// separately so a copy-paste between the two would be caught.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn built_in_connection_relation_cannot_be_edited_or_deleted(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-cr-bi@b.test", "Alice").await;

    // When
    let res = patch(
        app.clone(),
        &cookie,
        "/api/connection-relations/depends_on",
        serde_json::json!({ "label": "hijacked" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(message(&json_body(res).await).contains("builtin relations cannot be edited"));

    let res = delete(app.clone(), &cookie, "/api/connection-relations/depends_on").await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(message(&json_body(res).await).contains("builtin relations cannot be deleted"));

    let row = sqlx::query!(
        r#"SELECT label, COUNT(*) OVER () AS "n!" FROM connection_relation WHERE id = 'depends_on'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.label, "Depends on");
    assert_eq!(row.n, 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn connection_relation_cross_user_mutations_are_404_and_leave_the_row_alone(
    pool: sqlx::PgPool,
) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice = signup(app.clone(), "alice-cr-x@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-cr-x@b.test", "Bob").await;
    // When
    post(
        app.clone(),
        &alice,
        "/api/connection-relations",
        serde_json::json!({ "id": "alices", "label": "Alice's" }),
    )
    .await;

    // Then
    assert_eq!(
        patch(
            app.clone(),
            &bob,
            "/api/connection-relations/alices",
            serde_json::json!({ "label": "hijacked" })
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        delete(app.clone(), &bob, "/api/connection-relations/alices")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    let still: String =
        sqlx::query_scalar!("SELECT label FROM connection_relation WHERE id = 'alices'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(still, "Alice's");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_connection_relation_returns_message_deleted_and_removes_row(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-cr-del@b.test", "Alice").await;
    post(
        app.clone(),
        &cookie,
        "/api/connection-relations",
        serde_json::json!({ "id": "disposable", "label": "D" }),
    )
    .await;

    // When
    let res = delete(app.clone(), &cookie, "/api/connection-relations/disposable").await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({ "message": "Deleted" })
    );

    let count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM connection_relation WHERE id = 'disposable'"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

// --- auth -------------------------------------------------------------------

/// All eight endpoints sit behind `requireSession` in Node
/// (`packages/api/src/vocabularies/routes.ts`), so none of them may serve
/// an anonymous caller.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn every_endpoint_requires_a_session(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));

    let cases: Vec<(&str, &str)> = vec![
        ("GET", "/api/chunk-types"),
        ("POST", "/api/chunk-types"),
        ("PATCH", "/api/chunk-types/note"),
        ("DELETE", "/api/chunk-types/note"),
        ("GET", "/api/connection-relations"),
        ("POST", "/api/connection-relations"),
        ("PATCH", "/api/connection-relations/related_to"),
        ("DELETE", "/api/connection-relations/related_to"),
    ];

    for (method, path) in cases {
        // When
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"id":"x","label":"X"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Then
        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path} must require a session"
        );
    }
}
