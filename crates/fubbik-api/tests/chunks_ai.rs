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

/// A chunk with no embedding at all — the `embedding` column is left NULL,
/// the state a never-enriched chunk is in.
async fn seed_chunk_no_embedding(pool: &sqlx::PgPool, user_id: &str, id: &str, title: &str) {
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id)
         VALUES ($1, $2, 'content', 'note', $3)",
    )
    .bind(id)
    .bind(title)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
}

/// A 768-dimension pgvector literal with two non-zero components, letting
/// tests place a chunk at an arbitrary angle from a `one_hot` source vector
/// instead of only "identical" (distance 0) or "orthogonal" (distance 1).
/// Cosine distance ignores magnitude, so the weights only need to encode
/// direction.
fn weighted_pgvector_text(weights: &[(usize, f32)]) -> String {
    let mut parts = vec!["0".to_string(); 768];
    for (index, weight) in weights {
        parts[*index] = weight.to_string();
    }
    format!("[{}]", parts.join(","))
}

async fn seed_chunk_with_weighted_vector(
    pool: &sqlx::PgPool,
    user_id: &str,
    id: &str,
    title: &str,
    weights: &[(usize, f32)],
) {
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id, embedding)
         VALUES ($1, $2, 'content', 'note', $3, $4::text::vector)",
    )
    .bind(id)
    .bind(title)
    .bind(user_id)
    .bind(weighted_pgvector_text(weights))
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

/// Same as [`ollama_mock`], plus a `/api/tags` mock answering 200 — needed
/// on `check-similar`'s path, which probes availability
/// (`chunks::ai::check_similar`) before embedding, unlike
/// `semantic_search`.
async fn ollama_mock_available(vector: Vec<f32>) -> wiremock::MockServer {
    let server = ollama_mock(vector).await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/tags"))
        .respond_with(wiremock::ResponseTemplate::new(200))
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

/// A non-numeric `limit` falls back to the default of 5, not an error and
/// not "no cap" — pinned by seeding well more than 5 candidates and
/// asserting the exact count, so this would fail if the fallback silently
/// became the 20-cap or "all rows" instead of 5.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_with_non_numeric_limit_falls_back_to_default_of_five(pool: sqlx::PgPool) {
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
        "/api/chunks/search/semantic?q=whatever&limit=abc",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = json_body(res).await;
    assert_eq!(
        body.as_array().unwrap().len(),
        5,
        "a non-numeric limit must fall back to the default of 5, not 0, not 20, not all 25"
    );
}

/// `exclude=<term>` filters on `not_about`. The excluded chunk is seeded as
/// the *closer* match (hot index 0, matching the query vector) so that if
/// the filter were deleted, it would come back first — a test that only
/// checked the surviving chunk was present would still pass with the
/// filter gone.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_excludes_terms_in_not_about(pool: sqlx::PgPool) {
    let server = ollama_mock(one_hot(0)).await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_with_vector(&pool, &user_id, "near", "Near", 0).await;
    seed_chunk_with_vector(&pool, &user_id, "far", "Far", 5).await;
    sqlx::query("UPDATE chunk SET not_about = '[\"billing\"]'::jsonb WHERE id = 'near'")
        .execute(&pool)
        .await
        .unwrap();

    let res = get(
        app.clone(),
        &cookie,
        "/api/chunks/search/semantic?q=whatever&exclude=billing",
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
        "the closer 'near' chunk must be filtered out by exclude=billing, leaving only 'far'"
    );
}

// ---------------------------------------------------------------------------
// POST /api/chunks/check-similar
// ---------------------------------------------------------------------------

/// The call site passes threshold 0.75, not the repository's own default of
/// 0.7 (`similarity.ts:13-15`). An identical-vector chunk (similarity 1.0)
/// clears it; an orthogonal one (similarity 0.0) does not.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn check_similar_returns_only_matches_at_or_above_the_threshold(pool: sqlx::PgPool) {
    let server = ollama_mock_available(one_hot(0)).await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_with_vector(&pool, &user_id, "identical", "Identical", 0).await;
    seed_chunk_with_vector(&pool, &user_id, "orthogonal", "Orthogonal", 5).await;

    let res = post(
        app.clone(),
        &cookie,
        "/api/chunks/check-similar",
        serde_json::json!({ "title": "New chunk", "content": "some content" }),
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
        vec!["identical"],
        "only the chunk at/above the 0.75 threshold should come back"
    );
}

/// Node returns `[]`, not a 502, when Ollama is down here — `checkSimilar`
/// probes availability first (`similarity.ts:9`). This is the opposite of
/// semantic search, and the asymmetry is Node's.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn check_similar_returns_empty_when_ollama_is_unreachable(pool: sqlx::PgPool) {
    // Default client in `state()` points at port 1 — nothing listens there,
    // so `is_available` returns false without a real network call ever
    // reaching an embeddings endpoint.
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_with_vector(&pool, &user_id, "identical", "Identical", 0).await;

    let res = post(
        app.clone(),
        &cookie,
        "/api/chunks/check-similar",
        serde_json::json!({ "title": "New chunk", "content": "some content" }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "an unreachable Ollama must degrade to 200 + [], not 502"
    );

    let body = json_body(res).await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

/// The call site passes limit 3, not the repository's own default of 5
/// (`similarity.ts:13-15`). Five identical-vector chunks are seeded; only
/// three must come back.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn check_similar_caps_at_three(pool: sqlx::PgPool) {
    let server = ollama_mock_available(one_hot(0)).await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    for i in 0..5 {
        seed_chunk_with_vector(&pool, &user_id, &format!("c{i}"), &format!("C{i}"), 0).await;
    }

    let res = post(
        app.clone(),
        &cookie,
        "/api/chunks/check-similar",
        serde_json::json!({ "title": "New chunk", "content": "some content" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = json_body(res).await;
    assert_eq!(
        body.as_array().unwrap().len(),
        3,
        "5 identical-vector matches exist; the call-site limit of 3 must cap the response"
    );
}

/// `excludeId` (typically the chunk being edited) must be omitted from its
/// own similarity results even though it would otherwise match at 1.0.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn check_similar_omits_the_excluded_id(pool: sqlx::PgPool) {
    let server = ollama_mock_available(one_hot(0)).await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_with_vector(&pool, &user_id, "self", "Self", 0).await;
    seed_chunk_with_vector(&pool, &user_id, "other", "Other", 0).await;

    let res = post(
        app.clone(),
        &cookie,
        "/api/chunks/check-similar",
        serde_json::json!({ "title": "New chunk", "content": "some content", "excludeId": "self" }),
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
        vec!["other"],
        "excludeId=self must drop 'self' from the results even though it matches at 1.0"
    );
}

// ---------------------------------------------------------------------------
// GET /api/chunks/{id}/neighbors
// ---------------------------------------------------------------------------

/// No Ollama call at all on this path: the source chunk's stored embedding
/// is used, not a freshly generated one. `neighbors()` never references the
/// AI client on either the missing-embedding branch or the normal branch,
/// so the zero-requests assertion below holds either way and doesn't by
/// itself prove the early return — the `note` assertion above it is what
/// does that. It's kept anyway as a forward-looking guard: if a future
/// refactor adds an Ollama call to this path, this test should catch it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn neighbors_notes_a_missing_embedding_without_calling_ollama(pool: sqlx::PgPool) {
    let server = ollama_mock(one_hot(0)).await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_no_embedding(&pool, &user_id, "unenriched", "Unenriched").await;

    let res = get(app.clone(), &cookie, "/api/chunks/unenriched/neighbors").await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = json_body(res).await;
    assert_eq!(body["neighbors"].as_array().unwrap().len(), 0);
    assert_eq!(
        body["note"].as_str().unwrap(),
        "Chunk has no embedding — run enrichment first."
    );

    let requests = server.received_requests().await.unwrap();
    assert!(
        requests.is_empty(),
        "neighbors must never call Ollama, but got: {requests:?}"
    );
}

/// Three chunks at distinct angles from the source vector (not just
/// "identical" or "orthogonal"), so the ordering pins the actual
/// `combinedScore` sort, not just set membership.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn neighbors_are_ordered_by_combined_score(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_with_vector(&pool, &user_id, "source", "Source", 0).await;
    // Identical direction to source: distance 0, embeddingSimilarity 1.0.
    seed_chunk_with_weighted_vector(&pool, &user_id, "closest", "Closest", &[(0, 1.0)]).await;
    // 45 degrees off source: distance strictly between 0 and 1.
    seed_chunk_with_weighted_vector(&pool, &user_id, "middle", "Middle", &[(0, 0.5), (1, 0.5)])
        .await;
    // Orthogonal to source: distance 1, embeddingSimilarity 0.0.
    seed_chunk_with_vector(&pool, &user_id, "farthest", "Farthest", 5).await;

    let res = get(app.clone(), &cookie, "/api/chunks/source/neighbors").await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = json_body(res).await;
    assert!(body["note"].is_null());
    let ids: Vec<&str> = body["neighbors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["closest", "middle", "farthest"]);
}

/// `k` defaults to 10 and is clamped to `1..=50` (`chunks/routes.ts:303`).
/// `k=0` must not mean "no results" and `k=999` must not error.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn neighbors_k_is_clamped_between_one_and_fifty(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_with_vector(&pool, &user_id, "source", "Source", 0).await;
    for i in 0..3 {
        seed_chunk_with_vector(&pool, &user_id, &format!("n{i}"), &format!("N{i}"), i).await;
    }

    let res_zero = get(app.clone(), &cookie, "/api/chunks/source/neighbors?k=0").await;
    assert_eq!(res_zero.status(), StatusCode::OK);
    let body_zero = json_body(res_zero).await;
    assert_eq!(
        body_zero["neighbors"].as_array().unwrap().len(),
        1,
        "k=0 must be clamped up to 1, not treated as 'no results'"
    );

    let res_big = get(app.clone(), &cookie, "/api/chunks/source/neighbors?k=999").await;
    assert_eq!(
        res_big.status(),
        StatusCode::OK,
        "k=999 must be clamped down to 50, not passed straight to the query"
    );
    let body_big = json_body(res_big).await;
    assert_eq!(
        body_big["neighbors"].as_array().unwrap().len(),
        3,
        "only 3 candidate neighbors were seeded; clamping to 50 must not error"
    );
}

/// The 0.15 graph bonus must be able to REORDER, not just decorate: seed a
/// slightly-worse embedding match that is graph-connected and assert it
/// overtakes a slightly-better one that is not.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn the_graph_bonus_reorders_neighbours(pool: sqlx::PgPool) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "a@b.test", "A").await;
    let user_id = user_id_by_email(&pool, "a@b.test").await;

    seed_chunk_with_vector(&pool, &user_id, "source", "Source", 0).await;
    // Closer embedding match (embeddingSimilarity ~0.9986), but NOT
    // graph-connected. With no bonus, combinedScore stays ~0.9986.
    seed_chunk_with_weighted_vector(
        &pool,
        &user_id,
        "better_unconnected",
        "Better unconnected",
        &[(0, 0.95), (1, 0.05)],
    )
    .await;
    // Slightly worse embedding match (embeddingSimilarity ~0.9939), but IS
    // graph-connected. The two are deliberately close (~0.005 apart) so the
    // 0.15 bonus (-> combinedScore ~1.1439) comfortably overtakes the
    // unconnected chunk's ~0.9986 — a gap the bonus could never close if the
    // two were seeded farther apart.
    seed_chunk_with_weighted_vector(
        &pool,
        &user_id,
        "worse_connected",
        "Worse connected",
        &[(0, 0.9), (1, 0.1)],
    )
    .await;

    fubbik_db::age::ensure_vertex(&pool, "source")
        .await
        .unwrap();
    fubbik_db::age::ensure_vertex(&pool, "worse_connected")
        .await
        .unwrap();
    fubbik_db::age::create_edge(&pool, "related_to", "source", "worse_connected")
        .await
        .unwrap();

    let res = get(app.clone(), &cookie, "/api/chunks/source/neighbors").await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = json_body(res).await;
    let neighbors = body["neighbors"].as_array().unwrap();
    let worse_connected = neighbors
        .iter()
        .find(|n| n["id"] == "worse_connected")
        .expect("worse_connected must be present");
    let better_unconnected = neighbors
        .iter()
        .find(|n| n["id"] == "better_unconnected")
        .expect("better_unconnected must be present");

    assert!(
        worse_connected["graphConnected"].as_bool().unwrap(),
        "worse_connected must be flagged as graph-connected"
    );
    assert!(
        !better_unconnected["graphConnected"].as_bool().unwrap(),
        "better_unconnected must NOT be flagged as graph-connected"
    );
    assert!(
        worse_connected["embeddingSimilarity"].as_f64().unwrap()
            < better_unconnected["embeddingSimilarity"].as_f64().unwrap(),
        "worse_connected must genuinely have the worse raw embedding match"
    );
    assert!(
        worse_connected["combinedScore"].as_f64().unwrap()
            > better_unconnected["combinedScore"].as_f64().unwrap(),
        "the graph bonus must let worse_connected overtake better_unconnected on combinedScore"
    );

    let ids: Vec<&str> = neighbors
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids.first().copied(),
        Some("worse_connected"),
        "worse_connected must be ranked first after the bonus reorders it"
    );
}

// ---------------------------------------------------------------------------
// PATCH /api/chunks/{id} — re-enrich on title/content edit
// ---------------------------------------------------------------------------

/// Same shape as `enrich.rs`'s `ollama_mock`: an availability probe plus
/// generation and embedding endpoints, all needed because a re-enrich runs
/// the same full pipeline as a manual `/enrich` call.
async fn ollama_mock_for_reenrich() -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/tags"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/generate"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "{\"summary\":\"A summary.\",\"aliases\":[\"a1\",\"a2\"],\"notAbout\":[\"n1\"]}"
            })),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/embeddings"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "embedding": vec![0.01f32; 768] })),
        )
        .mount(&server)
        .await;
    server
}

/// The re-enrich is detached, so this polls with a deadline rather than
/// sleeping: a sleep long enough to be reliable is long enough to slow the
/// suite, and a short one is flaky.
async fn wait_for_summary(pool: &sqlx::PgPool, id: &str) -> Option<String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let row = sqlx::query!("SELECT summary FROM chunk WHERE id = $1", id)
            .fetch_one(pool)
            .await
            .unwrap();
        if row.summary.is_some() {
            return row.summary;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    None
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patching_the_title_triggers_a_re_enrich(pool: sqlx::PgPool) {
    let server = ollama_mock_for_reenrich().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = post(
        app.clone(),
        &cookie,
        "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" }),
    )
    .await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{id}"),
        serde_json::json!({ "title": "New title" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    assert_eq!(
        wait_for_summary(&pool, &id).await.as_deref(),
        Some("A summary."),
        "patching the title must trigger a detached re-enrich that writes the summary"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patching_the_content_triggers_a_re_enrich(pool: sqlx::PgPool) {
    let server = ollama_mock_for_reenrich().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = post(
        app.clone(),
        &cookie,
        "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" }),
    )
    .await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{id}"),
        serde_json::json!({ "content": "New content" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    assert_eq!(
        wait_for_summary(&pool, &id).await.as_deref(),
        Some("A summary."),
        "patching the content must trigger a detached re-enrich that writes the summary"
    );
}

/// Node gates on `title !== undefined || content !== undefined`
/// (`chunk-mutations.ts:213`). A PATCH of any other field must not spend an
/// Ollama call.
///
/// A poll-with-deadline can only prove a positive ("the summary showed up
/// eventually"); it cannot prove a negative, because "not yet" and "never"
/// look identical to a poll that times out. So this asserts directly on the
/// mock's request log instead: after the PATCH response comes back (and a
/// short settle window to let a wrongly-spawned task's first HTTP call land),
/// `received_requests()` must show no `/api/generate` hit. That is not
/// racy — a spawned re-enrich would either have already made the call by
/// then, or the test would need to wait forever for something that isn't
/// coming, which is exactly what "no re-enrich was triggered" means.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patching_another_field_does_not_trigger_a_re_enrich(pool: sqlx::PgPool) {
    let server = ollama_mock_for_reenrich().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = post(
        app.clone(),
        &cookie,
        "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" }),
    )
    .await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{id}"),
        serde_json::json!({ "rationale": "Some rationale" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    // Give a wrongly-spawned task a generous window to have made its first
    // Ollama call — this is not a poll for absence-of-evidence-as-proof, it
    // just bounds how long we wait before inspecting the mock's log.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let requests = server.received_requests().await.unwrap();
    let generate_calls: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path() == "/api/generate")
        .collect();
    assert!(
        generate_calls.is_empty(),
        "patching a non-title/content field must not call /api/generate, but got: {generate_calls:?}"
    );

    let row = sqlx::query!("SELECT summary FROM chunk WHERE id = $1", id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        row.summary.is_none(),
        "summary must stay NULL when no re-enrich was triggered"
    );
}

/// The spawned task must not be able to fail the request.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_failing_re_enrich_does_not_fail_the_patch(pool: sqlx::PgPool) {
    // Default client in `state()` points at port 1 — nothing listens there,
    // so `enrich_chunk` returns `Ok(None)` (Ollama unavailable) without
    // ever reaching a real network call.
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = post(
        app.clone(),
        &cookie,
        "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" }),
    )
    .await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{id}"),
        serde_json::json!({ "title": "New title" }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "a PATCH must return 200 even when the spawned re-enrich cannot reach Ollama"
    );

    let body = json_body(res).await;
    assert_eq!(body["id"].as_str().unwrap(), id);
    assert_eq!(body["title"].as_str().unwrap(), "New title");
}

/// The port-1 scenario above only exercises `enrich_chunk`'s `Ok(None)`
/// branch (Ollama unavailable) — it never reaches the `Err` branch that
/// `tracing::error!` actually logs. This test forces a *genuine* failure
/// (Ollama reachable, but `/api/generate` returns an undecodable body, so
/// `enrich_chunk` returns `Err`) to prove the PATCH is tolerant of that
/// path too, not just the "unavailable" one.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_genuinely_failing_re_enrich_does_not_fail_the_patch(pool: sqlx::PgPool) {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/tags"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/generate"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "response": "not valid json at all" })),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/embeddings"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "embedding": vec![0.01f32; 768] })),
        )
        .mount(&server)
        .await;

    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = post(
        app.clone(),
        &cookie,
        "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" }),
    )
    .await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{id}"),
        serde_json::json!({ "title": "New title" }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "a PATCH must return 200 even when the spawned re-enrich genuinely errors (not just when Ollama is unreachable)"
    );

    let body = json_body(res).await;
    assert_eq!(body["id"].as_str().unwrap(), id);
    assert_eq!(body["title"].as_str().unwrap(), "New title");
}

/// Pins that the PATCH response does not wait on the re-enrich call —
/// gap (b) from review: removing `tokio::spawn` and awaiting inline left
/// every other test in this file green, so nothing was pinning "must not
/// block on Ollama" until this test existed.
///
/// A dedicated `MockServer` (not the shared `ollama_mock_for_reenrich`
/// helper) because this one needs a `/api/generate` mock that stalls for 5
/// seconds — sharing that with tests that expect fast, synchronous-looking
/// enrichment would slow them down or make them racy for no reason, the
/// same logic that split `ollama_mock_available` from `ollama_mock`
/// earlier in this file.
///
/// Threshold: asserts the PATCH returns in well under 1 second. The only
/// work on the response path is a database UPDATE plus building the JSON
/// response — nothing that should approach 100ms even on slow CI, let alone
/// 1s — while the mutation this guards against (awaiting the re-enrich
/// inline) would take at least 5s, the mock's delay. A 1s bar leaves a 50x
/// margin against the failure case while still being generous enough that
/// no legitimate scheduling jitter could trip it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patching_the_title_returns_before_the_re_enrich_completes(pool: sqlx::PgPool) {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/tags"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/generate"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "response": "{\"summary\":\"A summary.\",\"aliases\":[\"a1\",\"a2\"],\"notAbout\":[\"n1\"]}"
                }))
                .set_delay(std::time::Duration::from_secs(5)),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/embeddings"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "embedding": vec![0.01f32; 768] })),
        )
        .mount(&server)
        .await;

    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = post(
        app.clone(),
        &cookie,
        "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" }),
    )
    .await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    let start = std::time::Instant::now();
    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/chunks/{id}"),
        serde_json::json!({ "title": "New title" }),
    )
    .await;
    let elapsed = start.elapsed();

    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "PATCH must not block on the re-enrich call — it took {elapsed:?}, \
         but the mock's /api/generate delay is 5s, so anything approaching \
         that means the response is waiting on the spawned task"
    );
}
