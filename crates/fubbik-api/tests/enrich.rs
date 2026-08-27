//! HTTP-level tests for `POST /api/chunks/{id}/enrich` and
//! `POST /api/chunks/enrich-all` (`packages/api/src/enrich/routes.ts`).

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

/// A 768-length embedding response, since the column is `vector(768)` and a
/// wrong-length vector is rejected by Postgres, not silently accepted.
fn embedding_body() -> serde_json::Value {
    serde_json::json!({ "embedding": vec![0.01f32; 768] })
}

async fn ollama_mock() -> wiremock::MockServer {
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
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(embedding_body()))
        .mount(&server)
        .await;
    server
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_writes_all_four_columns(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" }),
    )
    .await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/chunks/{id}/enrich"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let row = sqlx::query!(
        r#"SELECT summary, aliases, not_about,
                  embedding::text AS embedding,
                  embedding_updated_at
           FROM chunk WHERE id = $1"#,
        id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.summary.as_deref(), Some("A summary."));
    assert_eq!(row.aliases, serde_json::json!(["a1", "a2"]));
    assert_eq!(row.not_about, serde_json::json!(["n1"]));
    assert!(
        row.embedding.is_some(),
        "the embedding column must be written"
    );
    assert!(row.embedding_updated_at.is_some());
}

/// Node returns `null` and writes nothing when Ollama is unreachable
/// (`enrich/service.ts:16`). The endpoint must still be a 200 — the CLI's
/// enrich-all counts non-null results and treats a failure as a skip, not
/// an error.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_is_a_no_op_when_ollama_is_unreachable(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone())); // default client points at port 1
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" }),
    )
    .await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/chunks/{id}/enrich"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(json_body(res).await.is_null());

    let row = sqlx::query!("SELECT summary FROM chunk WHERE id = $1", id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        row.summary.is_none(),
        "nothing may be written when Ollama is down"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_404s_for_a_missing_chunk(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks/nope/enrich",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Ruling A: Node's `enrichChunk` looks the chunk up with **no** user id
/// (`enrich/service.ts:20`), so any authenticated user can trigger
/// enrichment on any other user's chunk today. Rust's `enrich_chunk` is
/// user-scoped by construction; this test is the only thing that pins that
/// tightening.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_404s_and_leaves_another_users_chunk_untouched(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);

    let cookie_a = signup(app.clone(), "a@b.test", "A").await;
    let cookie_b = signup(app.clone(), "c@d.test", "C").await;

    let created = send(
        app.clone(),
        &cookie_a,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": "A's chunk", "content": "C", "type": "note" }),
    )
    .await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    let res = send(
        app.clone(),
        &cookie_b,
        "POST",
        &format!("/api/chunks/{id}/enrich"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let row = sqlx::query!("SELECT summary FROM chunk WHERE id = $1", id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        row.summary.is_none(),
        "user B's enrich attempt must not touch user A's chunk"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_is_rate_limited_at_ten_per_minute(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" }),
    )
    .await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    for i in 0..10 {
        let res = send(
            app.clone(),
            &cookie,
            "POST",
            &format!("/api/chunks/{id}/enrich"),
            serde_json::Value::Null,
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK, "call {i}");
    }
    let res = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/chunks/{id}/enrich"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    let body = json_body(res).await;
    assert_eq!(body["error"], "Rate limit exceeded");
    assert!(body["retryAfter"].as_i64().unwrap() > 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_all_enriches_every_chunk_and_counts_them(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    for i in 0..3 {
        send(
            app.clone(),
            &cookie,
            "POST",
            "/api/chunks",
            serde_json::json!({ "title": format!("T{i}"), "content": "C", "type": "note" }),
        )
        .await;
    }

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks/enrich-all",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["enriched"], 3);

    let count = sqlx::query_scalar!("SELECT count(*) FROM chunk WHERE summary IS NOT NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(3));
}

/// One user's chunks must not be enriched by another user's sweep.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_all_is_scoped_to_the_caller(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);

    let cookie_a = signup(app.clone(), "a@b.test", "A").await;
    let cookie_b = signup(app.clone(), "c@d.test", "C").await;
    send(
        app.clone(),
        &cookie_b,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": "B's", "content": "C", "type": "note" }),
    )
    .await;

    let res = send(
        app.clone(),
        &cookie_a,
        "POST",
        "/api/chunks/enrich-all",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(json_body(res).await["enriched"], 0);

    let count = sqlx::query_scalar!("SELECT count(*) FROM chunk WHERE summary IS NOT NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(0), "A's sweep must not touch B's chunks");
}

/// Node's per-item `catchAll(() => Effect.succeed(null))` means one
/// chunk's failure must not abort the sweep — the other chunks still get
/// enriched. A body-matched mock returns 500 for exactly one chunk's
/// prompt (registered ahead of the catch-all 200, since wiremock resolves
/// mocks in registration order); the other two use the normal success
/// path. This asserts continuation itself (both survivors' summaries are
/// populated), not just the decremented count.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_all_continues_past_a_single_chunk_failure(pool: sqlx::PgPool) {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/tags"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;
    // The specific, failing mock must be registered BEFORE the catch-all
    // success mock below, or the catch-all would swallow every request.
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/generate"))
        .and(wiremock::matchers::body_string_contains("Title: Bravo"))
        .respond_with(wiremock::ResponseTemplate::new(500))
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
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(embedding_body()))
        .mount(&server)
        .await;

    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    // Distinct titles, chosen so none is a substring of another — unlike
    // "T1"/"T10", "Bravo" cannot accidentally match "Alpha" or "Charlie".
    let mut ids = std::collections::HashMap::new();
    for title in ["Alpha", "Bravo", "Charlie"] {
        let created = send(
            app.clone(),
            &cookie,
            "POST",
            "/api/chunks",
            serde_json::json!({ "title": title, "content": "C", "type": "note" }),
        )
        .await;
        let id = json_body(created).await["id"].as_str().unwrap().to_string();
        ids.insert(title, id);
    }

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks/enrich-all",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["enriched"], 2);

    let bravo_summary =
        sqlx::query_scalar!("SELECT summary FROM chunk WHERE id = $1", ids["Bravo"])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        bravo_summary.is_none(),
        "the chunk whose generate call failed must not have a summary"
    );

    for title in ["Alpha", "Charlie"] {
        let summary = sqlx::query_scalar!("SELECT summary FROM chunk WHERE id = $1", ids[title])
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(
            summary.is_some(),
            "{title}'s enrichment must have proceeded despite Bravo's failure"
        );
    }
}

/// `fubbik_db::repo::chunk::list_ids_for_user` must exclude archived
/// chunks, matching Node's `listChunks(userId, {...})` which pushes
/// `isNull(chunk.archivedAt)` whenever `includeArchived` is falsy
/// (`packages/db/src/repository/chunk.ts:37-39`). Without that filter an
/// archived chunk would get an embedding it never had under Node, and
/// would eat into the sweep's 1000-row budget.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_all_skips_archived_chunks(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let mut ids = Vec::new();
    for i in 0..3 {
        let created = send(
            app.clone(),
            &cookie,
            "POST",
            "/api/chunks",
            serde_json::json!({ "title": format!("T{i}"), "content": "C", "type": "note" }),
        )
        .await;
        ids.push(json_body(created).await["id"].as_str().unwrap().to_string());
    }

    let archived_id = ids[0].clone();
    let res = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/chunks/{archived_id}/archive"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks/enrich-all",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["enriched"], 2);

    let archived_summary =
        sqlx::query_scalar!("SELECT summary FROM chunk WHERE id = $1", archived_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        archived_summary.is_none(),
        "the archived chunk must not have been swept into enrich-all"
    );
}
