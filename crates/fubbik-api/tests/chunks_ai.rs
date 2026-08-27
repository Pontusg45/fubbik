//! HTTP-level tests for `GET /api/chunks/search/semantic`
//! (`packages/api/src/chunks/chunk-search.ts:59-73` +
//! `packages/api/src/chunks/routes.ts:192-206`).

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

/// A 768-dimension vector that is all zeros except one hot index, matching
/// the pattern in `crates/fubbik-db/tests/semantic.rs`. Cosine distance
/// between two such vectors is exactly 0 when the indices match and 1 when
/// they differ, so expected orderings are unambiguous.
fn one_hot(index: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; 768];
    v[index] = 1.0;
    v
}

fn one_hot_pgvector_text(index: usize) -> String {
    let mut parts = vec!["0"; 768];
    parts[index] = "1";
    format!("[{}]", parts.join(","))
}

/// Looks up the id of a user already created through the HTTP signup flow
/// (`signup`), so that seeded chunks are owned by the same user id the
/// session cookie authenticates as — direct-inserting a user row instead
/// would collide with signup's own insert on the unique email constraint.
async fn user_id_by_email(pool: &sqlx::PgPool, email: &str) -> String {
    fubbik_db::repo::user::find_by_email(pool, email)
        .await
        .unwrap()
        .expect("user must exist after signup")
        .id
}

async fn seed_chunk_with_vector(
    pool: &sqlx::PgPool,
    user_id: &str,
    id: &str,
    title: &str,
    hot: usize,
) {
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id, embedding)
         VALUES ($1, $2, 'content', 'note', $3, $4::text::vector)",
    )
    .bind(id)
    .bind(title)
    .bind(user_id)
    .bind(one_hot_pgvector_text(hot))
    .execute(pool)
    .await
    .unwrap();
}

/// Mounts a mock `/api/embeddings` that answers every request with the
/// given one-hot vector, regardless of the prompt. No `/api/tags` mock is
/// registered — this path has no availability probe (see
/// `chunks::ai::semantic_search`'s doc comment), so nothing should ever hit
/// that endpoint.
async fn ollama_mock(vector: Vec<f32>) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/embeddings"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "embedding": vector })),
        )
        .mount(&server)
        .await;
    server
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_returns_hits_ranked_by_similarity(pool: sqlx::PgPool) {
    let server = ollama_mock(one_hot(0)).await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_with_vector(&pool, &user_id, "far", "Far", 5).await;
    seed_chunk_with_vector(&pool, &user_id, "near", "Near", 0).await;

    let res = get(
        app.clone(),
        &cookie,
        "/api/chunks/search/semantic?q=whatever",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = json_body(res).await;
    let ids: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["id"].as_str().unwrap())
        .collect();
    // Order, not membership: a test that only checks both ids came back
    // would pass with the `ORDER BY` deleted.
    assert_eq!(ids, vec!["near", "far"]);
}

/// `limit` is capped at 20 (`chunk-search.ts:60`). A request for 100 must
/// not return 100.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_caps_limit_at_twenty(pool: sqlx::PgPool) {
    let server = ollama_mock(one_hot(0)).await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    for i in 0..25 {
        seed_chunk_with_vector(&pool, &user_id, &format!("c{i}"), &format!("C{i}"), i).await;
    }

    let res = get(
        app.clone(),
        &cookie,
        "/api/chunks/search/semantic?q=whatever&limit=100",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = json_body(res).await;
    assert_eq!(body.as_array().unwrap().len(), 20);
}

/// `scope="env:prod,garbage"` — the well-formed pair filters, the
/// malformed one is discarded rather than causing an error.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_parses_scope_pairs_and_drops_malformed_ones(pool: sqlx::PgPool) {
    let server = ollama_mock(one_hot(0)).await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_with_vector(&pool, &user_id, "near", "Near", 0).await;
    seed_chunk_with_vector(&pool, &user_id, "far", "Far", 5).await;
    sqlx::query("UPDATE chunk SET scope = '{\"env\":\"prod\"}'::jsonb WHERE id = 'far'")
        .execute(&pool)
        .await
        .unwrap();

    // If the malformed "garbage" entry (no colon) caused an error instead
    // of being discarded, this request would fail rather than filter down
    // to the `env:prod` chunk.
    let res = get(
        app.clone(),
        &cookie,
        "/api/chunks/search/semantic?q=whatever&scope=env:prod,garbage",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = json_body(res).await;
    let ids: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec!["far"],
        "only the chunk matching the well-formed env:prod pair should come back"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_is_rate_limited_at_thirty_per_minute(pool: sqlx::PgPool) {
    let server = ollama_mock(one_hot(0)).await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_with_vector(&pool, &user_id, "near", "Near", 0).await;

    for i in 0..30 {
        let res = get(
            app.clone(),
            &cookie,
            "/api/chunks/search/semantic?q=whatever",
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK, "call {i}");
    }
    let res = get(
        app.clone(),
        &cookie,
        "/api/chunks/search/semantic?q=whatever",
    )
    .await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    let body = json_body(res).await;
    assert_eq!(body["error"], "Rate limit exceeded");
    assert!(body["retryAfter"].as_i64().unwrap() > 0);
}

/// Node has no availability probe on this path — `generateQueryEmbedding`
/// failing propagates straight as an `AiError` -> 502. This must NOT come
/// back as an empty 200: an unreachable Ollama and "no matches" are
/// different situations and the caller must be able to tell them apart.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_502s_when_ollama_is_unreachable(pool: sqlx::PgPool) {
    // Default client in `state()` points at port 1 — nothing listens there.
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_with_vector(&pool, &user_id, "near", "Near", 0).await;

    let res = get(
        app.clone(),
        &cookie,
        "/api/chunks/search/semantic?q=whatever",
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
}
