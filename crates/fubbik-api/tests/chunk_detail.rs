//! HTTP-level tests for `GET /api/chunks/{id}` — the **enriched** detail
//! shape, a port of Node's `getChunkDetail`
//! (`packages/api/src/chunks/service.ts:128-186`).
//!
//! Until this landed, the route returned the bare `chunk` row, and that one
//! shape gap is what pinned the chunk detail page, the chunk edit page and
//! the graph side panel to `legacyApi` — the app's central surface.
//!
//! What is proven where: the SQL ownership guards behind each of the seven
//! sub-queries are proven at the repository layer (`fubbik-db/tests/
//! {connection,requirement,chunk_meta,feature,tag,space}.rs`), where a
//! removed guard is observed directly instead of through a status code.
//! This file proves the assembly: that every key is present, that the
//! health score is computed from the *enriched* inputs rather than
//! defaults, and that active feature overlays are applied in priority
//! order.

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
    serde_json::from_slice(&body).unwrap()
}

async fn post(
    app: axum::Router,
    cookie: &str,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post(path.to_string())
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn put(
    app: axum::Router,
    cookie: &str,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::put(path.to_string())
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_detail(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/chunks/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn create_chunk(app: axum::Router, cookie: &str, title: &str, content: &str) -> String {
    let res = post(
        app,
        cookie,
        "/api/chunks",
        serde_json::json!({ "title": title, "content": content }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    json_body(res).await["id"].as_str().unwrap().to_string()
}

/// Every key Node's `getChunkDetail` returns is present, and `chunk` is the
/// chunk itself rather than a wrapper.
///
/// The pre-port response was the bare `Chunk` row, so this test fails on
/// the very first assertion against it — which is the point: a client
/// reading `data.chunk.title` got `undefined`, silently.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_chunk_returns_the_enriched_detail_shape(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = create_chunk(app.clone(), &cookie, "Subject", "some content").await;

    let body = json_body(get_detail(app, &cookie, &id).await).await;

    assert_eq!(body["chunk"]["id"], id.as_str());
    assert_eq!(body["chunk"]["title"], "Subject");

    // Enumerated, not spot-checked: every key Node emits, including the two
    // underscore-prefixed ones and the redundant `allDeltas`/`deltas` pair.
    for key in [
        "chunk",
        "connections",
        "spaces",
        "appliesTo",
        "fileReferences",
        "tags",
        "requirements",
        "allDeltas",
        "healthScore",
        "_appliedFeatures",
        "_hasDeltas",
        "deltas",
    ] {
        assert!(
            body.get(key).is_some(),
            "detail response is missing the `{key}` key"
        );
    }

    // Empty collections must be `[]`, not absent and not `null` — the web
    // app maps over every one of these without a guard.
    for key in [
        "connections",
        "spaces",
        "appliesTo",
        "fileReferences",
        "tags",
        "requirements",
        "allDeltas",
        "deltas",
        "_appliedFeatures",
    ] {
        assert!(
            body[key].is_array() && body[key].as_array().unwrap().is_empty(),
            "`{key}` must be an empty array on a bare chunk, got {}",
            body[key]
        );
    }
    assert_eq!(body["_hasDeltas"], false);

    assert!(
        body["healthScore"]["total"].is_number(),
        "healthScore must be the object shape `{{total, breakdown, issues}}`"
    );
    for category in [
        "freshness",
        "completeness",
        "richness",
        "connectivity",
        "coverage",
    ] {
        assert!(
            body["healthScore"]["breakdown"][category].is_number(),
            "healthScore.breakdown is missing `{category}`"
        );
    }
    assert!(body["healthScore"]["issues"].is_array());
}

/// The sub-resources actually populate their arrays — a detail response
/// whose every array is empty would pass the shape test above while
/// enriching nothing.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detail_populates_connections_applies_to_and_file_refs(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let subject = create_chunk(app.clone(), &cookie, "Subject", "content").await;
    let neighbour = create_chunk(app.clone(), &cookie, "Neighbour", "content").await;

    let res = post(
        app.clone(),
        &cookie,
        "/api/connections",
        serde_json::json!({
            "sourceId": subject, "targetId": neighbour, "relation": "related_to"
        }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "connection must be created"
    );

    put(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{subject}/applies-to"),
        serde_json::json!([{ "pattern": "src/**/*.ts", "note": "typed only" }]),
    )
    .await;
    put(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{subject}/file-refs"),
        serde_json::json!([{ "path": "src/lib.rs", "anchor": "fn main", "relation": "implements" }]),
    )
    .await;

    let body = json_body(get_detail(app, &cookie, &subject).await).await;

    assert_eq!(body["connections"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["connections"][0]["title"], "Neighbour",
        "a connection carries the OTHER end's title"
    );
    assert_eq!(body["connections"][0]["relation"], "related_to");

    // The three columns the pre-port projection silently dropped.
    assert_eq!(body["appliesTo"][0]["pattern"], "src/**/*.ts");
    assert_eq!(body["appliesTo"][0]["note"], "typed only");
    assert_eq!(body["fileReferences"][0]["path"], "src/lib.rs");
    assert_eq!(body["fileReferences"][0]["anchor"], "fn main");
    assert_eq!(body["fileReferences"][0]["relation"], "implements");
}

/// `connectivity` is computed from the real connection count, not from the
/// zero the pre-port bare-row response implied.
///
/// Asserted as a *change* between two responses for the same chunk rather
/// than against a hardcoded number, so the test cannot silently start
/// passing for a chunk whose score was already at the asserted value.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn health_score_connectivity_reflects_real_connections(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let subject = create_chunk(app.clone(), &cookie, "Subject", "content").await;
    let neighbour = create_chunk(app.clone(), &cookie, "Neighbour", "content").await;

    let before = json_body(get_detail(app.clone(), &cookie, &subject).await).await;
    assert_eq!(
        before["healthScore"]["breakdown"]["connectivity"], 0,
        "an unconnected chunk scores zero for connectivity"
    );
    assert!(
        before["healthScore"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i == "Orphan chunk with no connections"),
        "an unconnected chunk must be flagged as an orphan"
    );

    post(
        app.clone(),
        &cookie,
        "/api/connections",
        serde_json::json!({
            "sourceId": subject, "targetId": neighbour, "relation": "related_to"
        }),
    )
    .await;

    let after = json_body(get_detail(app, &cookie, &subject).await).await;
    assert!(
        after["healthScore"]["breakdown"]["connectivity"]
            .as_i64()
            .unwrap()
            > 0,
        "connectivity must rise once the chunk has a connection"
    );
    assert!(
        after["healthScore"]["total"].as_i64().unwrap()
            > before["healthScore"]["total"].as_i64().unwrap(),
        "the total must rise with connectivity"
    );
}

/// Active feature deltas are applied to `chunk`, highest priority last, and
/// reported in `_appliedFeatures`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detail_applies_active_feature_overlays_in_priority_order(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = create_chunk(app.clone(), &cookie, "Base title", "base content").await;

    let low = json_body(
        post(
            app.clone(),
            &cookie,
            "/api/features",
            serde_json::json!({ "name": "low", "priority": 1 }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let high = json_body(
        post(
            app.clone(),
            &cookie,
            "/api/features",
            serde_json::json!({ "name": "high", "priority": 2 }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Both features rewrite `title`; only the low-priority one rewrites
    // `content`. So the result proves BOTH that the higher priority wins a
    // contested field and that the lower one is still applied at all — a
    // "last write wins" implementation that skipped the first delta
    // entirely would pass a title-only assertion.
    put(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{id}/deltas/{low}"),
        serde_json::json!({ "delta": { "title": "Low title", "content": "Low content" } }),
    )
    .await;
    put(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{id}/deltas/{high}"),
        serde_json::json!({ "delta": { "title": "High title" } }),
    )
    .await;

    // With no features active, the base chunk is served — but `_hasDeltas`
    // still reports that deltas exist.
    let inactive = json_body(get_detail(app.clone(), &cookie, &id).await).await;
    assert_eq!(inactive["chunk"]["title"], "Base title");
    assert_eq!(inactive["chunk"]["content"], "base content");
    assert_eq!(
        inactive["_hasDeltas"], true,
        "_hasDeltas counts ALL deltas, active or not"
    );
    assert!(
        inactive["_appliedFeatures"].as_array().unwrap().is_empty(),
        "_appliedFeatures counts only active features"
    );
    assert_eq!(
        inactive["deltas"].as_array().unwrap().len(),
        2,
        "`deltas` lists every delta regardless of which features are active"
    );
    assert_eq!(
        inactive["allDeltas"], inactive["deltas"],
        "`allDeltas` and `deltas` are the same list — Node ships both keys"
    );

    put(
        app.clone(),
        &cookie,
        "/api/features/active",
        serde_json::json!({ "featureIds": [low, high] }),
    )
    .await;

    let active = json_body(get_detail(app, &cookie, &id).await).await;
    assert_eq!(
        active["chunk"]["title"], "High title",
        "the higher-priority delta must win the contested field"
    );
    assert_eq!(
        active["chunk"]["content"], "Low content",
        "the lower-priority delta must still apply to fields nobody else set"
    );
    let applied: Vec<&str> = active["_appliedFeatures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        applied,
        [low.as_str(), high.as_str()],
        "_appliedFeatures is in application order: ascending priority"
    );
}

/// Linked requirements populate `requirements` and drive the `coverage`
/// half of the health score.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detail_reports_linked_requirements_and_their_coverage(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = create_chunk(app.clone(), &cookie, "Subject", "content").await;

    let before = json_body(get_detail(app.clone(), &cookie, &id).await).await;
    assert_eq!(before["healthScore"]["breakdown"]["coverage"], 0);

    // `POST /api/requirements` answers 201 with
    // `{ requirement, warnings, vocabularyWarnings }`, not a bare row.
    let created = post(
        app.clone(),
        &cookie,
        "/api/requirements",
        serde_json::json!({
            "title": "Users can log in",
            // A full Given/When/Then — the service rejects an incomplete
            // BDD triple with a 400.
            "steps": [
                { "keyword": "given", "text": "a user" },
                { "keyword": "when", "text": "they log in" },
                { "keyword": "then", "text": "they see the dashboard" }
            ]
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let req = json_body(created).await["requirement"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = put(
        app.clone(),
        &cookie,
        &format!("/api/requirements/{req}/chunks"),
        serde_json::json!({ "chunkIds": [id] }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK, "chunk link must be created");

    let after = json_body(get_detail(app, &cookie, &id).await).await;
    assert_eq!(after["requirements"].as_array().unwrap().len(), 1);
    assert_eq!(after["requirements"][0]["title"], "Users can log in");
    assert_eq!(after["requirements"][0]["chunkId"], id.as_str());
    assert_eq!(
        after["requirements"][0]["status"], "untested",
        "a fresh requirement is untested, so coverage must not reach its top tier"
    );
    assert!(
        after["healthScore"]["breakdown"]["coverage"]
            .as_i64()
            .unwrap()
            > 0,
        "coverage must rise once a requirement is linked"
    );
}

/// Another user's chunk is a 404, not an enriched body.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detail_is_404_for_another_users_chunk(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;
    let id = create_chunk(app.clone(), &alice, "Alice's chunk", "content").await;

    assert_eq!(
        get_detail(app.clone(), &alice, &id).await.status(),
        StatusCode::OK,
        "the owner must get a 200 — otherwise the assertion below could \
         pass for the wrong reason"
    );
    assert_eq!(
        get_detail(app, &bob, &id).await.status(),
        StatusCode::NOT_FOUND
    );
}
