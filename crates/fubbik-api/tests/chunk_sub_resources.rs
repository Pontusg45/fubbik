//! Cross-user isolation for the applies-to and file-ref sub-resources.
//!
//! Every user-scoped surface in this project has a cross-user test proving
//! `service::get`'s ownership check actually holds at the HTTP boundary —
//! these four endpoints (`GET|PUT .../applies-to`, `GET|PUT .../file-refs`)
//! did not. Drives real HTTP requests through the router with two real,
//! signed-up users so a future refactor that drops the `service::get` call
//! from a handler fails a test, not just a review.

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
/// `tests/auth.rs::cookie_pair`.
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

/// Creates a chunk as the user identified by `cookie` and returns its id.
async fn create_chunk(app: axum::Router, cookie: &str, title: &str) -> String {
    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(format!(r#"{{"title":"{title}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "chunk creation must succeed"
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn put_applies_to(
    app: axum::Router,
    cookie: &str,
    id: &str,
    patterns: &[&str],
) -> axum::response::Response {
    // A **bare array** of `{pattern, note}` objects — Node's shape, and now
    // Rust's. This helper omits `note` (the common case); the tests that
    // care about it build their own body.
    let body = serde_json::Value::Array(
        patterns
            .iter()
            .map(|p| serde_json::json!({ "pattern": p }))
            .collect(),
    )
    .to_string();
    app.oneshot(
        Request::put(format!("/api/chunks/{id}/applies-to"))
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_applies_to(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/chunks/{id}/applies-to"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn put_file_refs(
    app: axum::Router,
    cookie: &str,
    id: &str,
    paths: &[&str],
) -> axum::response::Response {
    // Bare array, as with `put_applies_to`. `relation` is required by Node's
    // schema, so the helper supplies the default.
    let body = serde_json::Value::Array(
        paths
            .iter()
            .map(|p| serde_json::json!({ "path": p, "relation": "documents" }))
            .collect(),
    )
    .to_string();
    app.oneshot(
        Request::put(format!("/api/chunks/{id}/file-refs"))
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_file_refs(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/chunks/{id}/file-refs"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn patterns_json(response: axum::response::Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_get_applies_to_is_404(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));

    let alice_cookie = signup(app.clone(), "alice-ato-get@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-ato-get@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;

    // When
    let res = get_applies_to(app.clone(), &bob_cookie, &chunk_id).await;
    // Then
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to read this chunk's applies-to patterns"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_put_applies_to_is_404_and_leaves_patterns_unchanged(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));

    let alice_cookie = signup(app.clone(), "alice-ato-put@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-ato-put@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;

    // When
    let seed = put_applies_to(app.clone(), &alice_cookie, &chunk_id, &["src/**/*.ts"]).await;
    // Then
    assert_eq!(seed.status(), StatusCode::OK);

    let res = put_applies_to(app.clone(), &bob_cookie, &chunk_id, &["evil/**"]).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to overwrite this chunk's applies-to patterns"
    );

    // A 404 that still mutated the row would be worse than a 200: prove
    // Alice's patterns are byte-for-byte unchanged after Bob's rejected PUT.
    let after = patterns_json(get_applies_to(app.clone(), &alice_cookie, &chunk_id).await).await;
    let patterns: Vec<&str> = after
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["pattern"].as_str().unwrap())
        .collect();
    assert_eq!(
        patterns,
        vec!["src/**/*.ts"],
        "Alice's patterns must survive Bob's rejected PUT"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_get_file_refs_is_404(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));

    let alice_cookie = signup(app.clone(), "alice-fr-get@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-fr-get@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;

    // When
    let res = get_file_refs(app.clone(), &bob_cookie, &chunk_id).await;
    // Then
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to read this chunk's file refs"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_put_file_refs_is_404_and_leaves_refs_unchanged(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));

    let alice_cookie = signup(app.clone(), "alice-fr-put@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-fr-put@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;

    // When
    let seed = put_file_refs(app.clone(), &alice_cookie, &chunk_id, &["src/index.ts"]).await;
    // Then
    assert_eq!(seed.status(), StatusCode::OK);

    let res = put_file_refs(app.clone(), &bob_cookie, &chunk_id, &["evil.ts"]).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to overwrite this chunk's file refs"
    );

    // A 404 that still mutated the row would be worse than a 200: prove
    // Alice's file refs are byte-for-byte unchanged after Bob's rejected PUT.
    let after = patterns_json(get_file_refs(app.clone(), &alice_cookie, &chunk_id).await).await;
    let paths: Vec<&str> = after
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec!["src/index.ts"],
        "Alice's file refs must survive Bob's rejected PUT"
    );
}

/// Sends a raw JSON body to one of the two sub-resource PUTs — used by the
/// tests below that need a body this file's helpers don't build (a `note`,
/// an `anchor`, a non-default `relation`, or a deliberately invalid one).
async fn raw_put(
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

/// `note` survives a write/read round trip.
///
/// This is the regression test for a silent data-loss bug: the
/// `chunk_applies_to.note` column has existed since `0001_init.sql:141` and
/// Node returns it, but Rust's projection selected only
/// `id`/`chunk_id`/`pattern`, so a note the user typed was written by Node,
/// invisible through Rust, and erased by the next Rust write. Fails on the
/// pre-fix code at the `note` assertion, not at the status code — the
/// endpoint looked healthy the whole time.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn applies_to_note_round_trips(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "T").await;

    // When
    let res = raw_put(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{chunk_id}/applies-to"),
        serde_json::json!([
            { "pattern": "src/**/*.ts", "note": "only the typed ones" },
            { "pattern": "docs/**" }
        ]),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);

    // `get_applies_to` orders by `pattern, id`, so `docs/**` sorts first.
    let rows = patterns_json(get_applies_to(app, &cookie, &chunk_id).await).await;
    assert_eq!(rows[0]["pattern"], "docs/**");
    assert_eq!(
        rows[0]["note"],
        serde_json::Value::Null,
        "an omitted note must read back as null, not as an empty string"
    );
    assert_eq!(rows[1]["pattern"], "src/**/*.ts");
    assert_eq!(
        rows[1]["note"], "only the typed ones",
        "the note must survive the round trip — it was dropped by the \
         projection before the chunk-detail port"
    );
}

/// `anchor` and `relation` survive a write/read round trip — the file-ref
/// half of [`applies_to_note_round_trips`], and the same class of bug.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn file_refs_anchor_and_relation_round_trip(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "T").await;

    // When
    let res = raw_put(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{chunk_id}/file-refs"),
        serde_json::json!([
            { "path": "src/lib.rs", "anchor": "fn main", "relation": "implements" },
            { "path": "README.md", "relation": "documents" }
        ]),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);

    let rows = patterns_json(get_file_refs(app, &cookie, &chunk_id).await).await;
    // Ordered by `path, id`: README.md, then src/lib.rs.
    assert_eq!(rows[0]["path"], "README.md");
    assert_eq!(rows[0]["anchor"], serde_json::Value::Null);
    assert_eq!(rows[0]["relation"], "documents");
    assert_eq!(rows[1]["path"], "src/lib.rs");
    assert_eq!(
        rows[1]["anchor"], "fn main",
        "the anchor must survive the round trip"
    );
    assert_eq!(
        rows[1]["relation"], "implements",
        "a non-default relation must survive the round trip — before the \
         chunk-detail port every ref read back as the column default"
    );
}

/// `relation` is constrained to Node's four literals, and the rejection is
/// a 400 naming the field rather than a serde parse error.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn file_refs_put_rejects_an_unknown_relation(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "T").await;

    // When
    let res = raw_put(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{chunk_id}/file-refs"),
        serde_json::json!([{ "path": "src/lib.rs", "relation": "vandalises" }]),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // The rejection must be total: nothing from the batch was written.
    let rows = patterns_json(get_file_refs(app, &cookie, &chunk_id).await).await;
    assert_eq!(
        rows.as_array().unwrap().len(),
        0,
        "a rejected batch must not write any of its entries"
    );
}

/// Node caps both sub-resource bodies at 50 entries; so does this.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn applies_to_put_rejects_more_than_fifty_entries(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "T").await;

    let fifty: Vec<serde_json::Value> = (0..50)
        .map(|i| serde_json::json!({ "pattern": format!("src/{i}/**") }))
        .collect();
    let mut fifty_one = fifty.clone();
    fifty_one.push(serde_json::json!({ "pattern": "one/too/many/**" }));

    let path = format!("/api/chunks/{chunk_id}/applies-to");

    // When
    // 51 is rejected...
    let res = raw_put(
        app.clone(),
        &cookie,
        &path,
        serde_json::Value::Array(fifty_one),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // ...and 50 is not, so the boundary is where it claims to be rather
    // than the test passing because everything is rejected.
    let res = raw_put(app, &cookie, &path, serde_json::Value::Array(fifty)).await;
    assert_eq!(res.status(), StatusCode::OK);
}
