//! HTTP-level tests for `density`, `timeline`, `file-refs` and `scope-keys`.
//!
//! The tree-building and range-parsing logic is unit-tested next to the code
//! (`density::service`, `timeline::routes`), where the interesting cases —
//! distinct-vs-summed subtree counts, malformed ranges — are reachable
//! without a database. This file proves the wiring: that the queries feed
//! those functions the right rows, and that everything is user-scoped.

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
    assert_eq!(res.status(), StatusCode::OK);
    res.headers()
        .get("set-cookie")
        .unwrap()
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

async fn send(
    app: axum::Router,
    cookie: &str,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookie);
    if !body.is_null() {
        req = req.header("content-type", "application/json");
    }
    let b = if body.is_null() {
        Body::empty()
    } else {
        Body::from(body.to_string())
    };
    app.oneshot(req.body(b).unwrap()).await.unwrap()
}

async fn get(app: axum::Router, cookie: &str, path: &str) -> axum::response::Response {
    send(app, cookie, "GET", path, serde_json::Value::Null).await
}

async fn a_chunk(app: axum::Router, cookie: &str, title: &str) -> String {
    let res = send(
        app,
        cookie,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": title, "content": "c" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    json_body(res).await["id"].as_str().unwrap().to_string()
}

// ---------------------------------------------------------------------------
// Density
// ---------------------------------------------------------------------------

/// Both sources feed the tree, and an applies-to glob is truncated at its
/// first wildcard — `src/**/*.ts` becomes the directory `src`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn density_builds_a_tree_from_globs_and_file_refs(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let a = a_chunk(app.clone(), &cookie, "A").await;
    let b = a_chunk(app.clone(), &cookie, "B").await;

    send(
        app.clone(),
        &cookie,
        "PUT",
        &format!("/api/chunks/{a}/applies-to"),
        serde_json::json!([{ "pattern": "src/**/*.ts" }]),
    )
    .await;
    send(
        app.clone(),
        &cookie,
        "PUT",
        &format!("/api/chunks/{b}/file-refs"),
        serde_json::json!([{ "path": "src/deep/thing.rs", "relation": "documents" }]),
    )
    .await;

    // When
    let body = json_body(get(app, &cookie, "/api/density").await).await;
    // Then
    assert_eq!(body["totals"]["chunksCovered"], 2);
    assert_eq!(body["totals"]["pathsTracked"], 2);

    let src = &body["tree"]["children"][0];
    assert_eq!(src["name"], "src");
    assert_eq!(
        src["chunkCount"], 2,
        "both chunks are counted in the subtree"
    );
    assert_eq!(
        src["directChunkCount"], 1,
        "only the glob-truncated one attaches to `src` itself"
    );
    assert_eq!(src["chunks"][0]["source"], "applies_to");

    let deep = &src["children"][0];
    assert_eq!(deep["name"], "deep");
    assert_eq!(deep["children"][0]["chunks"][0]["source"], "file_ref");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn density_excludes_archived_and_other_users_chunks(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;

    let kept = a_chunk(app.clone(), &alice, "Kept").await;
    let archived = a_chunk(app.clone(), &alice, "Archived").await;
    for id in [&kept, &archived] {
        send(
            app.clone(),
            &alice,
            "PUT",
            &format!("/api/chunks/{id}/file-refs"),
            serde_json::json!([{ "path": "src/x.rs", "relation": "documents" }]),
        )
        .await;
    }
    send(
        app.clone(),
        &alice,
        "POST",
        &format!("/api/chunks/{archived}/archive"),
        serde_json::Value::Null,
    )
    .await;

    // When
    let body = json_body(get(app.clone(), &alice, "/api/density").await).await;
    // Then
    assert_eq!(
        body["totals"]["chunksCovered"], 1,
        "an archived chunk drops out of the density view"
    );

    let bobs = json_body(get(app, &bob, "/api/density").await).await;
    assert_eq!(bobs["totals"]["chunksCovered"], 0);
    assert!(bobs["tree"]["children"].as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Timeline
// ---------------------------------------------------------------------------

/// A create produces a `created` event; an edit adds an `updated` one,
/// because "updated" means a version snapshot was written.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn timeline_reports_creates_and_edits(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = a_chunk(app.clone(), &cookie, "Tracked").await;

    // When
    let body = json_body(get(app.clone(), &cookie, "/api/timeline").await).await;
    // Then
    assert_eq!(body["totals"]["created"], 1);
    assert_eq!(body["totals"]["updated"], 0);
    assert_eq!(body["range"]["days"], 30, "the default window");
    assert!(body["range"]["from"].is_string());

    send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/chunks/{id}"),
        serde_json::json!({ "title": "Edited" }),
    )
    .await;

    let body = json_body(get(app.clone(), &cookie, "/api/timeline?range=7d").await).await;
    assert_eq!(body["totals"]["created"], 1);
    assert_eq!(body["totals"]["updated"], 1);
    assert_eq!(body["range"]["days"], 7);

    let kinds: Vec<&str> = body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"created") && kinds.contains(&"updated"));

    let updated = body["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "updated")
        .unwrap();
    assert!(
        updated["version"].is_number(),
        "an updated event carries its version number; a created one does not"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn timeline_filters_by_tag_and_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;

    send(
        app.clone(),
        &alice,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": "Tagged", "content": "c", "tags": ["keeper"] }),
    )
    .await;
    a_chunk(app.clone(), &alice, "Untagged").await;

    // When
    let all = json_body(get(app.clone(), &alice, "/api/timeline").await).await;
    // Then
    assert_eq!(all["totals"]["created"], 2);

    let filtered = json_body(get(app.clone(), &alice, "/api/timeline?tag=keeper").await).await;
    assert_eq!(filtered["totals"]["created"], 1);
    assert_eq!(filtered["events"][0]["chunkTitle"], "Tagged");

    let bobs = json_body(get(app, &bob, "/api/timeline").await).await;
    assert_eq!(bobs["totals"]["created"], 0);
}

// ---------------------------------------------------------------------------
// File refs
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn file_ref_lookup_and_list_are_user_scoped(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;
    let id = a_chunk(app.clone(), &alice, "Documented").await;

    send(
        app.clone(),
        &alice,
        "PUT",
        &format!("/api/chunks/{id}/file-refs"),
        serde_json::json!([{ "path": "src/lib.rs", "anchor": "fn run", "relation": "implements" }]),
    )
    .await;

    // When
    let hits =
        json_body(get(app.clone(), &alice, "/api/file-refs/lookup?path=src/lib.rs").await).await;
    // Then
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert_eq!(hits[0]["chunkTitle"], "Documented");
    assert_eq!(hits[0]["anchor"], "fn run");
    assert_eq!(hits[0]["relation"], "implements");

    assert!(
        json_body(
            get(
                app.clone(),
                &alice,
                "/api/file-refs/lookup?path=src/other.rs"
            )
            .await
        )
        .await
        .as_array()
        .unwrap()
        .is_empty(),
        "an unreferenced path matches nothing — the lookup is exact, not prefix"
    );

    let all = json_body(get(app.clone(), &alice, "/api/file-refs").await).await;
    assert_eq!(all.as_array().unwrap().len(), 1);

    for path in ["/api/file-refs", "/api/file-refs/lookup?path=src/lib.rs"] {
        assert!(
            json_body(get(app.clone(), &bob, path).await)
                .await
                .as_array()
                .unwrap()
                .is_empty(),
            "{path} must not expose Alice's refs to Bob"
        );
    }
}

// ---------------------------------------------------------------------------
// Scope keys
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn scope_keys_round_trip_and_are_user_scoped(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;

    // When
    let res = send(
        app.clone(),
        &alice,
        "POST",
        "/api/scope-keys",
        serde_json::json!({ "key": "env", "valueType": "enum", "allowedValues": ["dev", "prod"] }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::CREATED);
    let created = json_body(res).await;
    assert_eq!(created["key"], "env");
    assert_eq!(created["allowedValues"], serde_json::json!(["dev", "prod"]));
    let id = created["id"].as_str().unwrap().to_string();

    // valueType defaults to `string`, and allowedValues stays null.
    let plain = json_body(
        send(
            app.clone(),
            &alice,
            "POST",
            "/api/scope-keys",
            serde_json::json!({ "key": "owner" }),
        )
        .await,
    )
    .await;
    assert_eq!(plain["valueType"], "string");
    assert_eq!(plain["allowedValues"], serde_json::Value::Null);

    let list = json_body(get(app.clone(), &alice, "/api/scope-keys").await).await;
    let keys: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["env", "owner"], "ordered by key");

    assert!(
        json_body(get(app.clone(), &bob, "/api/scope-keys").await)
            .await
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        send(
            app.clone(),
            &bob,
            "DELETE",
            &format!("/api/scope-keys/{id}"),
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::NOT_FOUND,
        "Bob must not delete Alice's key"
    );

    let res = send(
        app.clone(),
        &alice,
        "DELETE",
        &format!("/api/scope-keys/{id}"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Deleted");
    assert_eq!(
        json_body(get(app, &alice, "/api/scope-keys").await)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// An `enum` key with no values would leave the autocomplete offering an
/// empty list forever. Node does not check this; rejecting it is a
/// deliberate divergence.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn scope_key_validation(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;

    for (label, body) in [
        ("blank key", serde_json::json!({ "key": "  " })),
        (
            "unknown valueType",
            serde_json::json!({ "key": "k", "valueType": "vibes" }),
        ),
        (
            "enum with no allowedValues",
            serde_json::json!({ "key": "k", "valueType": "enum" }),
        ),
        (
            "enum with empty allowedValues",
            serde_json::json!({ "key": "k", "valueType": "enum", "allowedValues": [] }),
        ),
    ] {
        // When
        let res = send(app.clone(), &cookie, "POST", "/api/scope-keys", body).await;
        // Then
        assert_eq!(
            res.status(),
            StatusCode::BAD_REQUEST,
            "must reject: {label}"
        );
    }
}
