//! HTTP-level tests for the `search` domain: 6 endpoints under
//! `/api/search`. See `crates/fubbik-api/src/search/service.rs`'s module
//! doc for the two headline behaviours this file exists to pin: `POST
//! /api/search/query` always answers 200 (database failures degrade to
//! `{chunks: [], total: 0}`), and `DELETE /api/search/saved/{id}` never
//! 404s.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use fubbik_api::search::dto::SearchQueryBody;
use fubbik_api::search::parser::QueryClause;
use fubbik_db::repo::{chunk, connection, saved_query, tag, user};
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

/// Signs up a fresh user and returns the `name=value` session cookie pair,
/// matching the pattern in `tests/favorites.rs::signup`.
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

async fn seed_chunk(pool: &sqlx::PgPool, user_id: &str, title: &str, content: &str) -> String {
    chunk::create(
        pool,
        user_id,
        chunk::NewChunk {
            title: title.into(),
            content: content.into(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id
}

async fn get(app: axum::Router, path: &str, cookie: &str) -> axum::response::Response {
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
    path: &str,
    cookie: &str,
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

async fn delete_at(app: axum::Router, path: &str, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(path)
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

// ── GET /api/search/parse ──────────────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn parse_returns_the_raw_clause_array(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "parse@b.test", "P").await;

    let res = get(app, "/api/search/parse?q=type%3Areference", &cookie).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(
        body,
        serde_json::json!({
            "clauses": [
                {"field": "type", "operator": "is", "value": "reference"}
            ]
        }),
        "must be {{clauses: [...]}}, not a normalised string or a verdict"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn parse_requires_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let res = app
        .oneshot(
            Request::get("/api/search/parse?q=type:note")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

// ── POST /api/search/query ─────────────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn query_filters_by_type_tag_and_text(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "query-filters@b.test", "Q").await;
    let uid = user_id_for_email(&pool, "query-filters@b.test").await;

    let matching = seed_chunk(&pool, &uid, "Auth Flow", "about authentication flows").await;
    let t = tag::create(&pool, &uid, "auth", None).await.unwrap();
    tag::set_chunk_tags(&pool, &uid, &matching, std::slice::from_ref(&t.id))
        .await
        .unwrap();
    seed_chunk(&pool, &uid, "Unrelated", "something else entirely").await;

    let res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [
            {"field": "type", "operator": "is", "value": "note"},
            {"field": "tag", "operator": "is", "value": "auth"},
            {"field": "text", "operator": "contains", "value": "authentication"}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let chunks = body["chunks"].as_array().unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0]["id"], matching);
    assert_eq!(body["total"], 1);
    assert!(
        body.get("graphMeta").is_none() || body["graphMeta"].is_null(),
        "graphMeta must be absent when no graph clause is present"
    );
}

/// `POST /api/search/query` is scoped through `chunk::list`/`chunk::count`,
/// which are user-scoped in SQL (proven load-bearing in the `chunks`
/// domain's own test suite) — this is the search domain's own end-to-end
/// proof that a caller's search never surfaces another user's chunks, even
/// with an unfiltered query.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn query_never_returns_another_users_chunks(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "query-cross-alice@b.test", "Alice").await;
    let alice_id = user_id_for_email(&pool, "query-cross-alice@b.test").await;
    let bob_id = {
        signup(app.clone(), "query-cross-bob@b.test", "Bob").await;
        user_id_for_email(&pool, "query-cross-bob@b.test").await
    };
    seed_chunk(&pool, &alice_id, "Alice's chunk", "alice content").await;
    seed_chunk(&pool, &bob_id, "Bob's chunk", "bob content").await;

    let res = post(
        app,
        "/api/search/query",
        &alice_cookie,
        serde_json::json!({"clauses": []}),
    )
    .await;
    let body = json_body(res).await;
    let titles: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Alice's chunk"]);
}

/// User decision (Phase 2c final review, Fix 2): align `POST
/// /api/search/query`'s limit clamp to the same 100 cap `GET /api/chunks`
/// applies (`chunks::dto::ListChunksQuery::into_params`, divergence #11) —
/// both chunk-listing endpoints now share one cap, enforced at
/// `chunk::list`'s own `LIMIT` clause (`crates/fubbik-db/src/repo/chunk.rs`),
/// which is the *only* clamp `search::service::build_list_params` ever hits
/// (nothing pre-clamps upstream the way `GET /api/chunks` does). Node has no
/// cap on the search path at all, so `limit: 1000` is a documented
/// divergence going forward: Node returns every matching row, this port 100.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn query_limit_is_clamped_to_100(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "query-limit-cap@b.test", "L").await;
    let uid = user_id_for_email(&pool, "query-limit-cap@b.test").await;

    for i in 0..105 {
        seed_chunk(&pool, &uid, &format!("Chunk {i}"), "content").await;
    }

    let res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [], "limit": 1000}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(
        body["chunks"].as_array().unwrap().len(),
        100,
        "limit: 1000 must clamp down to 100, matching GET /api/chunks's own cap"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn query_connections_gte_filters_by_connection_count(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "query-conn@b.test", "Q").await;
    let uid = user_id_for_email(&pool, "query-conn@b.test").await;

    let connected = seed_chunk(&pool, &uid, "Connected", "has a connection").await;
    let other = seed_chunk(&pool, &uid, "Other", "the other end").await;
    let lonely = seed_chunk(&pool, &uid, "Lonely", "no connections at all").await;
    connection::create(
        &pool,
        &fubbik_db::new_id(),
        &uid,
        &connected,
        &other,
        "related_to",
        "human",
        "approved",
    )
    .await
    .unwrap();

    let res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [
            {"field": "connections", "operator": "gte", "value": "1"}
        ]}),
    )
    .await;
    let body = json_body(res).await;
    let ids: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&connected.as_str()));
    assert!(ids.contains(&other.as_str()));
    assert!(
        !ids.contains(&lonely.as_str()),
        "the lonely chunk has 0 connections and must be filtered out"
    );
}

/// `connections:abc+` yields a non-numeric value. Node's `Number("abc")` is
/// `NaN`, and `listChunks`'s `if (params.minConnections && params.minConnections
/// > 0)` guard is falsy for `NaN` — the filter is never applied, not
/// "matches nothing". This proves the port's choice: a lonely chunk (0
/// connections) still comes back.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn query_connections_with_a_non_numeric_value_applies_no_filter(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "query-conn-nan@b.test", "Q").await;
    let uid = user_id_for_email(&pool, "query-conn-nan@b.test").await;
    let lonely = seed_chunk(&pool, &uid, "Lonely", "no connections at all").await;

    let res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [
            {"field": "connections", "operator": "gte", "value": "abc"}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let ids: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&lonely.as_str()),
        "a non-numeric connections value must apply no filter at all, not exclude everything"
    );
}

/// `join` is dead in Node: accepted by both route schemas, never read by
/// `executeSearch`. `and` and `or` must produce byte-identical results.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn join_or_is_accepted_and_produces_identical_results_to_and(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "query-join@b.test", "Q").await;
    let uid = user_id_for_email(&pool, "query-join@b.test").await;
    seed_chunk(&pool, &uid, "One", "content one").await;
    seed_chunk(&pool, &uid, "Two", "content two").await;

    let and_res = post(
        app.clone(),
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [], "join": "and"}),
    )
    .await;
    let or_res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [], "join": "or"}),
    )
    .await;
    assert_eq!(and_res.status(), StatusCode::OK);
    assert_eq!(or_res.status(), StatusCode::OK);
    assert_eq!(
        json_body(and_res).await,
        json_body(or_res).await,
        "`join` is dead in Node — accepted by the schema, never read"
    );
}

/// A graph clause (`near`/`path`/`affected-by`/`similar-to`) that resolves
/// to zero ids makes the whole query degrade to an empty result, exactly
/// like Node does when its graph resolver returns zero ids — not an error,
/// not a clause silently dropped and the rest of the query still run.
/// `"some-id"` has no vertex in the graph at all, so `near:` resolves to no
/// neighbours; `graphMeta` still comes back (Node sets it the moment the
/// clause runs, not the moment it finds something).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn query_with_a_near_clause_that_resolves_to_no_ids_returns_empty(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "query-graph@b.test", "Q").await;
    let uid = user_id_for_email(&pool, "query-graph@b.test").await;
    seed_chunk(&pool, &uid, "Some chunk", "content").await;

    let res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [
            {"field": "near", "operator": "is", "value": "some-id", "params": {"hops": "2"}}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["chunks"], serde_json::json!([]));
    assert_eq!(body["total"], 0);
    assert_eq!(body["graphMeta"]["type"], "neighborhood");
    assert_eq!(body["graphMeta"]["referenceChunk"], "some-id");
}

// ── Task 9: the four graph clauses ─────────────────────────────────────

/// `near:` resolves via `age::get_neighborhood`, defaulting `hops` to 1
/// when the parser attached none, and reports `graphMeta.type ==
/// "neighborhood"` plus a per-chunk `hopDistance` context.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn near_clause_resolves_a_connected_chunk_and_sets_graph_meta(pool: sqlx::PgPool) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "near-basic@b.test", "N").await;
    let uid = user_id_for_email(&pool, "near-basic@b.test").await;
    let a = seed_chunk(&pool, &uid, "A", "content a").await;
    let b = seed_chunk(&pool, &uid, "B", "content b").await;
    fubbik_db::age::ensure_vertex(&pool, &a).await.unwrap();
    fubbik_db::age::ensure_vertex(&pool, &b).await.unwrap();
    fubbik_db::age::create_edge(&pool, "related_to", &a, &b)
        .await
        .unwrap();

    let res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [
            {"field": "near", "operator": "is", "value": a}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let ids: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![b.as_str()],
        "near: with no hops must default to 1 hop"
    );
    assert_eq!(body["graphMeta"]["type"], "neighborhood");
    assert_eq!(body["graphMeta"]["referenceChunk"], a);
    assert_eq!(
        body["chunks"][0]["graphContext"]["hopDistance"], 1,
        "hop distance falls back to the effective hop count (1, the default)"
    );
}

/// The one place in this port where ids arrive from outside a scoped SQL
/// query: an AGE `:connects` edge knows nothing about `user_id`, so it can
/// point straight at another user's chunk. `chunk::list`'s unconditional
/// `user_id = ..` predicate must still exclude it even though the graph
/// resolver itself found it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn near_clause_must_not_leak_another_users_chunk_across_a_graph_edge(pool: sqlx::PgPool) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "near-cross-alice@b.test", "Alice").await;
    let alice_id = user_id_for_email(&pool, "near-cross-alice@b.test").await;
    let bob_id = {
        signup(app.clone(), "near-cross-bob@b.test", "Bob").await;
        user_id_for_email(&pool, "near-cross-bob@b.test").await
    };
    let alices_chunk = seed_chunk(&pool, &alice_id, "Alice's chunk", "mine").await;
    let bobs_chunk = seed_chunk(&pool, &bob_id, "Bob's chunk", "not mine").await;
    fubbik_db::age::ensure_vertex(&pool, &alices_chunk)
        .await
        .unwrap();
    fubbik_db::age::ensure_vertex(&pool, &bobs_chunk)
        .await
        .unwrap();
    // A graph edge spanning two different users' chunks — AGE has no
    // concept of ownership, so this is legal at the graph layer.
    fubbik_db::age::create_edge(&pool, "related_to", &alices_chunk, &bobs_chunk)
        .await
        .unwrap();

    let res = post(
        app,
        "/api/search/query",
        &alice_cookie,
        serde_json::json!({"clauses": [
            {"field": "near", "operator": "is", "value": alices_chunk}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let ids: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(
        !ids.contains(&bobs_chunk.as_str()),
        "AGE resolved bob's chunk id via the graph edge, but chunk::list's user_id predicate must still exclude it"
    );
}

/// `affected-by:` resolves via `age::get_chunks_affected_by_requirement`,
/// defaulting `hops` to 2, and reports `graphMeta.type ==
/// "requirement-reach"`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn affected_by_clause_resolves_chunks_covered_by_a_requirement(pool: sqlx::PgPool) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "affected-basic@b.test", "A").await;
    let uid = user_id_for_email(&pool, "affected-basic@b.test").await;
    let covered = seed_chunk(&pool, &uid, "Covered", "content").await;
    fubbik_db::age::ensure_vertex(&pool, &covered)
        .await
        .unwrap();

    let requirement_id = fubbik_db::new_id();
    fubbik_db::age::cypher(
        &pool,
        &format!(
            "MERGE (:requirement {{id: '{}'}})",
            fubbik_db::age::esc_cypher(&requirement_id)
        ),
    )
    .await
    .unwrap();
    fubbik_db::age::cypher(
        &pool,
        &format!(
            "MATCH (r:requirement {{id: '{}'}}), (c:chunk {{id: '{}'}}) CREATE (r)-[:covers]->(c)",
            fubbik_db::age::esc_cypher(&requirement_id),
            fubbik_db::age::esc_cypher(&covered)
        ),
    )
    .await
    .unwrap();

    let res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [
            {"field": "affected-by", "operator": "is", "value": requirement_id}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let ids: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![covered.as_str()]);
    assert_eq!(body["graphMeta"]["type"], "requirement-reach");
}

/// `path:` resolves via `age::find_shortest_path_with_details`, using the
/// parser's `params.from`/`params.to` (not `clause.value.split(",")` —
/// see `search::service`'s module doc), and reports `graphMeta.type ==
/// "path"` with `pathChunks`/`pathEdges`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn path_clause_resolves_the_chunk_chain_and_edges(pool: sqlx::PgPool) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "path-basic@b.test", "P").await;
    let uid = user_id_for_email(&pool, "path-basic@b.test").await;
    let a = seed_chunk(&pool, &uid, "A", "content a").await;
    let b = seed_chunk(&pool, &uid, "B", "content b").await;
    fubbik_db::age::ensure_vertex(&pool, &a).await.unwrap();
    fubbik_db::age::ensure_vertex(&pool, &b).await.unwrap();
    fubbik_db::age::create_edge(&pool, "depends_on", &a, &b)
        .await
        .unwrap();

    let res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [
            {"field": "path", "operator": "is", "value": a, "params": {"from": a, "to": b}}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let ids: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    let mut expected = vec![a.as_str(), b.as_str()];
    expected.sort();
    assert_eq!(sorted, expected);
    assert_eq!(body["graphMeta"]["type"], "path");
    assert_eq!(body["graphMeta"]["pathChunks"], serde_json::json!([a, b]));
    assert_eq!(body["graphMeta"]["pathEdges"][0]["relation"], "depends_on");
}

/// `affected-by:` resolves ids via `age::get_chunks_affected_by_requirement`,
/// which has no ownership notion at all — a `:covers` edge can point
/// straight at another user's chunk. `chunk::list`'s mandatory `user_id =
/// ..` predicate (proved directly at the repo layer by
/// `tests/chunk.rs::list_with_ids_filter_cannot_leak_another_users_chunk`)
/// is what keeps it out of the response here; this test pins the
/// end-to-end HTTP behaviour, including that the id doesn't leak via any
/// other field in the body.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn affected_by_clause_must_not_leak_another_users_chunk_across_a_graph_edge(
    pool: sqlx::PgPool,
) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "affected-cross-alice@b.test", "Alice").await;
    let bob_id = {
        signup(app.clone(), "affected-cross-bob@b.test", "Bob").await;
        user_id_for_email(&pool, "affected-cross-bob@b.test").await
    };
    let bobs_chunk = seed_chunk(&pool, &bob_id, "Bob's chunk", "not mine").await;
    fubbik_db::age::ensure_vertex(&pool, &bobs_chunk)
        .await
        .unwrap();

    let requirement_id = fubbik_db::new_id();
    fubbik_db::age::cypher(
        &pool,
        &format!(
            "MERGE (:requirement {{id: '{}'}})",
            fubbik_db::age::esc_cypher(&requirement_id)
        ),
    )
    .await
    .unwrap();
    // A requirement covering another user's chunk — AGE has no concept of
    // ownership, so this is legal at the graph layer.
    fubbik_db::age::cypher(
        &pool,
        &format!(
            "MATCH (r:requirement {{id: '{}'}}), (c:chunk {{id: '{}'}}) CREATE (r)-[:covers]->(c)",
            fubbik_db::age::esc_cypher(&requirement_id),
            fubbik_db::age::esc_cypher(&bobs_chunk)
        ),
    )
    .await
    .unwrap();

    let res = post(
        app,
        "/api/search/query",
        &alice_cookie,
        serde_json::json!({"clauses": [
            {"field": "affected-by", "operator": "is", "value": requirement_id}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(
        body["chunks"],
        serde_json::json!([]),
        "the graph resolved bob's chunk, but chunk::list's user_id predicate must still exclude it, leaving nothing on the page"
    );
    let raw = body.to_string();
    assert!(
        !raw.contains(&bobs_chunk),
        "bob's chunk id must not appear anywhere in the response body, graphMeta included: {raw}"
    );
}

/// `path:`'s shortest-path search can route *through* another user's chunk
/// even when both endpoints belong to the caller — AGE walks edges with no
/// ownership notion at all. Divergence: this is a live disclosure this port
/// introduced (`path:` is dead code in Node — see the module doc), fixed by
/// `chunk::filter_visible_ids` in `resolve_graph_clauses`'s `path` arm.
/// Both the hidden chunk's id (`pathChunks`) and both edges touching it
/// (`pathEdges`) must be gone from `graphMeta`, not just from `chunks`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn path_clause_must_not_leak_another_users_chunk_in_graph_meta(pool: sqlx::PgPool) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "path-cross-alice@b.test", "Alice").await;
    let alice_id = user_id_for_email(&pool, "path-cross-alice@b.test").await;
    let bob_id = {
        signup(app.clone(), "path-cross-bob@b.test", "Bob").await;
        user_id_for_email(&pool, "path-cross-bob@b.test").await
    };
    let a = seed_chunk(&pool, &alice_id, "A", "content a").await;
    let b = seed_chunk(&pool, &alice_id, "B", "content b").await;
    // Only route from A to B passes through Bob's hidden chunk.
    let hidden = seed_chunk(&pool, &bob_id, "Bob's midpoint", "not mine").await;
    fubbik_db::age::ensure_vertex(&pool, &a).await.unwrap();
    fubbik_db::age::ensure_vertex(&pool, &b).await.unwrap();
    fubbik_db::age::ensure_vertex(&pool, &hidden).await.unwrap();
    fubbik_db::age::create_edge(&pool, "related_to", &a, &hidden)
        .await
        .unwrap();
    fubbik_db::age::create_edge(&pool, "related_to", &hidden, &b)
        .await
        .unwrap();

    let res = post(
        app,
        "/api/search/query",
        &alice_cookie,
        serde_json::json!({"clauses": [
            {"field": "path", "operator": "is", "value": a, "params": {"from": a, "to": b}}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;

    let chunk_ids: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(
        !chunk_ids.contains(&hidden.as_str()),
        "hidden chunk must not appear in chunks"
    );

    assert_eq!(body["graphMeta"]["type"], "path");
    let path_chunks: Vec<&str> = body["graphMeta"]["pathChunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        !path_chunks.contains(&hidden.as_str()),
        "graphMeta.pathChunks must not disclose the hidden chunk's id: {path_chunks:?}"
    );
    let edges = body["graphMeta"]["pathEdges"].as_array().unwrap();
    assert!(
        edges
            .iter()
            .all(|e| e["source"] != hidden.as_str() && e["target"] != hidden.as_str()),
        "graphMeta.pathEdges must drop every edge touching the hidden chunk: {edges:?}"
    );

    let raw = body.to_string();
    assert!(
        !raw.contains(&hidden),
        "hidden chunk id must not appear anywhere in the response body, graphMeta included: {raw}"
    );
}

/// `similar-to:` degrades to zero ids when Ollama is unreachable — this
/// test's `state()` points `ai` at `http://127.0.0.1:1`, nothing is
/// listening there, so `embed_query` always fails and the clause resolves
/// to `[]` (see `similar_to_degrades_to_empty_when_ollama_is_down` for the
/// same behaviour pinned directly at the service layer). `graphMeta.type`
/// must still come back as the literal string `"semantic"`, the fourth
/// value Node's own three-literal TS union doesn't declare (see the
/// module doc). Must not fail the suite even though nothing resembling
/// Ollama is running in this environment.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn similar_to_clause_degrades_to_empty_but_sets_graph_meta_type_semantic(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "similar-basic@b.test", "S").await;

    let res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [
            {"field": "similar-to", "operator": "is", "value": "authentication flow"}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["chunks"], serde_json::json!([]));
    assert_eq!(body["total"], 0);
    assert_eq!(body["graphMeta"]["type"], "semantic");
    assert_eq!(body["graphMeta"]["referenceChunk"], "authentication flow");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn query_requires_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let res = app
        .oneshot(
            Request::post("/api/search/query")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"clauses":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

/// The headline behaviour: `POST /api/search/query` always answers 200,
/// even when the underlying database call fails outright. Tested directly
/// at the service layer (not over HTTP) with a closed pool, which makes
/// every `sqlx` call on it fail deterministically and immediately — an
/// HTTP-level equivalent would also break session lookup (which needs the
/// same pool), conflating "the query failed" with "auth failed".
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_failing_query_degrades_to_empty_results_not_a_500(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "degrade@b.test", "D", None)
        .await
        .unwrap()
        .id;
    pool.close().await;

    let result = fubbik_api::search::service::execute_search(
        &pool,
        &fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
        &uid,
        &SearchQueryBody {
            clauses: vec![QueryClause {
                field: "type".into(),
                operator: "is".into(),
                value: "note".into(),
                params: None,
                negate: None,
            }],
            join: None,
            sort: None,
            limit: None,
            offset: None,
            space_id: None,
        },
    )
    .await;

    assert_eq!(result.chunks, vec![]);
    assert_eq!(result.total, 0);
}

// ── GET /api/search/autocomplete ───────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn autocomplete_tag_is_case_insensitive_prefix_and_capped_at_10(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "autotag@b.test", "A").await;
    let uid = user_id_for_email(&pool, "autotag@b.test").await;

    for name in [
        "Authentication",
        "architecture",
        "activity",
        "auth-flow",
        "auth-guard",
        "auth-token",
        "auth-scope",
        "auth-session",
        "auth-role",
        "auth-policy",
        "auth-realm",
        "unrelated",
    ] {
        tag::create(&pool, &uid, name, None).await.unwrap();
    }

    let res = get(
        app,
        "/api/search/autocomplete?field=tag&prefix=aut",
        &cookie,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let names: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(body.is_array(), "must be a bare string[]");
    assert!(names.len() <= 10, "must be capped at 10");
    assert!(
        names.iter().all(|n| n.to_lowercase().starts_with("aut")),
        "must be a case-insensitive prefix match: {names:?}"
    );
    assert!(
        !names.contains(&"architecture"),
        "'architecture' does not start with 'aut'"
    );
    assert!(!names.contains(&"unrelated"));
}

/// `tag` autocomplete goes through `tag::list(user_id)`, which is
/// user-scoped in SQL — proven load-bearing in `tests/tag.rs` already, but
/// this is the search domain's own end-to-end proof that a caller's tag
/// autocomplete never surfaces another user's tags. See
/// `autocomplete_chunk_never_returns_another_users_chunk_title` below for
/// the equivalent proof on the `chunk` branch (divergence #17, Phase 2c
/// task 8b).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn autocomplete_tag_never_returns_another_users_tags(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "autotag-cross-alice@b.test", "Alice").await;
    let alice_id = user_id_for_email(&pool, "autotag-cross-alice@b.test").await;
    let bob_id = {
        signup(app.clone(), "autotag-cross-bob@b.test", "Bob").await;
        user_id_for_email(&pool, "autotag-cross-bob@b.test").await
    };
    tag::create(&pool, &alice_id, "auth-alice", None)
        .await
        .unwrap();
    tag::create(&pool, &bob_id, "auth-bob", None).await.unwrap();

    let res = get(
        app,
        "/api/search/autocomplete?field=tag&prefix=auth",
        &alice_cookie,
    )
    .await;
    let body = json_body(res).await;
    let names: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["auth-alice"]);
}

/// `chunk`/`requirement` autocomplete is `ILIKE '%prefix%'` — contains, not
/// a prefix match, matching Node's `searchChunkTitles`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn autocomplete_chunk_matches_contains_not_prefix(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "autochunk@b.test", "A").await;
    let uid = user_id_for_email(&pool, "autochunk@b.test").await;
    seed_chunk(&pool, &uid, "The Great Authentication Flow", "content").await;
    seed_chunk(&pool, &uid, "Unrelated Title", "content").await;

    let res = get(
        app,
        "/api/search/autocomplete?field=chunk&prefix=Authentication",
        &cookie,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        vec!["The Great Authentication Flow"],
        "the prefix appears mid-title, so only a contains match finds it"
    );
}

/// Divergence #17 (Phase 2c task 8b): Node's `searchChunkTitles` has no
/// `user_id` filter, so `GET /api/search/autocomplete?field=chunk` leaks
/// every user's chunk titles from a keystroke in the nav search bar. This
/// is the end-to-end proof of the fix: querying as Bob for a prefix that
/// only matches Alice's chunk title comes back empty; querying as Alice
/// still finds it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn autocomplete_chunk_never_returns_another_users_chunk_title(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    signup(app.clone(), "autochunk-cross-alice@b.test", "Alice").await;
    let alice_id = user_id_for_email(&pool, "autochunk-cross-alice@b.test").await;
    let bob_cookie = signup(app.clone(), "autochunk-cross-bob@b.test", "Bob").await;
    seed_chunk(&pool, &alice_id, "The Great Authentication Flow", "content").await;

    let res = get(
        app,
        "/api/search/autocomplete?field=chunk&prefix=Authentication",
        &bob_cookie,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!([]),
        "must not surface another user's chunk title"
    );
}

/// `requirement` autocomplete queries a table with no CRUD API in this
/// port yet, so it's always empty — matching Node against an empty table.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn autocomplete_requirement_is_empty(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "autoreq@b.test", "A").await;

    let res = get(
        app,
        "/api/search/autocomplete?field=requirement&prefix=any",
        &cookie,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await, serde_json::json!([]));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn autocomplete_of_an_unknown_field_is_empty(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "autounknown@b.test", "A").await;

    let res = get(
        app,
        "/api/search/autocomplete?field=bogus&prefix=x",
        &cookie,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await, serde_json::json!([]));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn autocomplete_requires_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let res = app
        .oneshot(
            Request::get("/api/search/autocomplete?field=tag&prefix=a")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

// ── GET/POST/DELETE /api/search/saved ──────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_then_list_saved_query_round_trips(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "saved-crud@b.test", "S").await;

    let res = post(
        app.clone(),
        "/api/search/saved",
        &cookie,
        serde_json::json!({
            "name": "my saved query",
            "query": {"clauses": [{"field": "type", "operator": "is", "value": "note"}]}
        }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let created = json_body(res).await;
    assert!(created.is_object(), "POST must return a bare object");
    assert_eq!(created["name"], "my saved query");

    let res = get(app, "/api/search/saved", &cookie).await;
    assert_eq!(res.status(), StatusCode::OK);
    let list = json_body(res).await;
    assert!(list.is_array(), "GET must return a bare array");
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["id"], created["id"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn duplicate_saved_query_names_are_allowed_over_http(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "saved-dup@b.test", "S").await;
    let body = serde_json::json!({"name": "same", "query": {"clauses": []}});

    let first = post(app.clone(), "/api/search/saved", &cookie, body.clone()).await;
    assert_eq!(first.status(), StatusCode::OK);
    let second = post(app.clone(), "/api/search/saved", &cookie, body).await;
    assert_eq!(
        second.status(),
        StatusCode::OK,
        "there is no unique constraint on (user_id, name)"
    );

    let list = json_body(get(app, "/api/search/saved", &cookie).await).await;
    assert_eq!(list.as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_saved_queries_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "saved-alice@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "saved-bob@b.test", "Bob").await;

    post(
        app.clone(),
        "/api/search/saved",
        &alice_cookie,
        serde_json::json!({"name": "alice's", "query": {"clauses": []}}),
    )
    .await;
    post(
        app.clone(),
        "/api/search/saved",
        &bob_cookie,
        serde_json::json!({"name": "bob's", "query": {"clauses": []}}),
    )
    .await;

    let alice_list = json_body(get(app.clone(), "/api/search/saved", &alice_cookie).await).await;
    assert_eq!(alice_list.as_array().unwrap().len(), 1);
    assert_eq!(alice_list[0]["name"], "alice's");

    let bob_list = json_body(get(app, "/api/search/saved", &bob_cookie).await).await;
    assert_eq!(bob_list.as_array().unwrap().len(), 1);
    assert_eq!(bob_list[0]["name"], "bob's");
}

/// Node ignores the delete result and always answers `{"message":"Deleted"}`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn deleting_a_nonexistent_saved_query_still_returns_200_deleted(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "saved-del-none@b.test", "S").await;

    let res = delete_at(app, "/api/search/saved/does-not-exist", &cookie).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({"message": "Deleted"}),
        "Node ignores the delete result and never 404s here"
    );
}

/// The delete is user-scoped in SQL even though the response can never
/// reveal it — this is the only test that can prove it, via a
/// surviving-row assertion rather than a status code.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn deleting_another_users_saved_query_returns_200_but_deletes_nothing(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "saved-del-alice@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "saved-del-bob@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "saved-del-alice@b.test").await;
    let alices = saved_query::create(&pool, &alice_id, "mine", serde_json::json!({}), None)
        .await
        .unwrap();

    let res = delete_at(
        app,
        &format!("/api/search/saved/{}", alices.id),
        &bob_cookie,
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "the response is indistinguishable — that is Node's behaviour"
    );
    assert_eq!(
        json_body(res).await,
        serde_json::json!({"message": "Deleted"})
    );

    let mine = saved_query::list(&pool, &alice_id, None).await.unwrap();
    assert_eq!(
        mine.len(),
        1,
        "but the row must survive: the DELETE is user-scoped in SQL"
    );

    // Sanity: alice can still delete it herself.
    let res = delete_at(
        fubbik_api::router(state(pool.clone())),
        &format!("/api/search/saved/{}", alices.id),
        &alice_cookie,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        saved_query::list(&pool, &alice_id, None)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn saved_requires_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/search/saved")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/search/saved")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"x","query":{"clauses":[]}}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = app
        .oneshot(
            Request::delete("/api/search/saved/some-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

// ── `similar-to:` clause (Task 11) ─────────────────────────────────────

/// A 768-dimension vector that is all zeros except one hot index, matching
/// the pattern in `crates/fubbik-db/tests/semantic.rs` and
/// `tests/chunks_ai.rs`. Cosine distance between two such vectors is
/// exactly 0 when the indices match and 1 when they differ, so expected
/// orderings/membership are unambiguous rather than approximate.
fn one_hot_pgvector_text(index: usize) -> String {
    let mut parts = vec!["0"; 768];
    parts[index] = "1";
    format!("[{}]", parts.join(","))
}

fn one_hot(index: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; 768];
    v[index] = 1.0;
    v
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

/// A 768-dimension vector that is all zeros except one index set to `-1`
/// — the antipode of `one_hot(index)`. Cosine distance from `one_hot(index)`
/// is 2 (maximally dissimilar), strictly worse than any orthogonal
/// (distance-1) vector, so it can be used to build a chunk guaranteed to
/// rank last against a same-index query.
fn opposite_pgvector_text(index: usize) -> String {
    let mut parts = vec!["0"; 768];
    parts[index] = "-1";
    format!("[{}]", parts.join(","))
}

async fn seed_chunk_with_opposite_vector(
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
    .bind(opposite_pgvector_text(hot))
    .execute(pool)
    .await
    .unwrap();
}

/// Mounts a mock `/api/embeddings` that answers every request with the
/// given vector, regardless of the prompt. No `/api/tags` mock is
/// registered — this clause has no availability probe (see
/// `search::service`'s module doc), so nothing should ever hit that
/// endpoint.
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

/// `similar-to:` has resolved to zero ids since this port began, because
/// no embedding pipeline existed (`search/service.rs:399`'s comment, since
/// rewritten). This is the first test that proves it resolves to
/// something real: two chunks are seeded at opposite one-hot vectors, the
/// mocked embedding for the query text matches the "near" chunk exactly,
/// and only "near" must come back — a stub returning every chunk (or
/// nothing) would fail this, unlike a membership-only check.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn similar_to_resolves_real_ids(pool: sqlx::PgPool) {
    let server = ollama_mock(one_hot(0)).await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "similar-real@b.test", "S").await;
    let user_id = user_id_for_email(&pool, "similar-real@b.test").await;

    // `semantic_search` has no similarity floor — it is a plain top-K
    // nearest-neighbour query (`fubbik_db::repo::semantic::semantic_search`'s
    // `ORDER BY ... LIMIT $5`, no `WHERE similarity > ...`). Two chunks
    // alone would both come back regardless of distance, since both fit
    // within the clause's `limit: 20`. To make "near" vs. "far" a real
    // discriminator, this seeds 20 filler chunks strictly closer to the
    // query than "far": one exact match ("near", cosine distance 0) plus
    // 20 orthogonal fillers (distance 1) already fill every one of the 20
    // slots, so "far" — placed at the opposite pole of the query vector
    // (cosine distance 2, worse than every filler) — can never make the
    // cut. "near" is always rank 1, so it always does.
    seed_chunk_with_vector(&pool, &user_id, "near", "Near", 0).await;
    for i in 1..=20 {
        seed_chunk_with_vector(&pool, &user_id, &format!("filler{i}"), "Filler", i).await;
    }
    seed_chunk_with_opposite_vector(&pool, &user_id, "far", "Far", 0).await;

    let res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [
            {"field": "similar-to", "operator": "is", "value": "authentication flow"}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["graphMeta"]["type"], "semantic");

    let ids: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&"near"),
        "the chunk whose vector matches the mocked embedding must be resolved: {ids:?}"
    );
    assert!(
        !ids.contains(&"far"),
        "the orthogonal chunk must not be resolved: {ids:?}"
    );
}

/// Node wraps the whole resolution in `Effect.orElse(() => [])`
/// (`search/service.ts:117-121`), so an unreachable Ollama yields an empty
/// clause, not an error — unlike the standalone semantic endpoint
/// (`GET /api/chunks/search/semantic`), which surfaces the same failure as
/// a 502. Pinned directly at the service layer against an `OllamaClient`
/// pointed at a port nothing listens on, so the failure is deterministic
/// rather than depending on network timing.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn similar_to_degrades_to_empty_when_ollama_is_down(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "similar-down@b.test", "S", None)
        .await
        .unwrap()
        .id;
    seed_chunk(&pool, &uid, "Some chunk", "content").await;

    let result = fubbik_api::search::service::execute_search(
        &pool,
        &fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
        &uid,
        &SearchQueryBody {
            clauses: vec![QueryClause {
                field: "similar-to".into(),
                operator: "is".into(),
                value: "authentication flow".into(),
                params: None,
                negate: None,
            }],
            join: None,
            sort: None,
            limit: None,
            offset: None,
            space_id: None,
        },
    )
    .await;

    assert_eq!(result.chunks, vec![]);
    assert_eq!(result.total, 0);
    assert_eq!(
        result.graph_meta.as_ref().map(|m| m.meta_type.as_str()),
        Some("semantic"),
        "graphMeta.type must still be the literal \"semantic\" even when Ollama is unreachable"
    );
}

/// A 768-dimension vector graded by `rank` (1-based): component `0` (the
/// query's own axis) holds `26 - rank`, and a distinct per-rank component
/// (index `rank`) holds `rank`. Neither component is ever zero for
/// `rank` in `1..=25`, so — unlike `one_hot`, whose off-axis vectors are
/// all mutually orthogonal (tied at cosine distance 1) — every graded
/// vector is a genuine, non-orthogonal candidate at a distinct distance
/// from `one_hot(0)`.
///
/// Let `x` denote `26 minus rank` and `y` denote `rank`, both strictly
/// positive across the whole range this function accepts. Cosine
/// similarity to `one_hot(0)` then equals `x` divided by the square root
/// of `x` squared plus `y` squared, which is the same as one divided by
/// the square root of one plus the square of `y` divided by `x`. That
/// ratio (`y` divided by `x`) grows strictly monotonically as `rank`
/// grows, since its numerator rises while its denominator falls, so
/// similarity falls strictly monotonically in lockstep with it: every
/// rank is strictly worse than the one before it, a genuine tie-free
/// total order rather than a pile of vectors tied at the same distance.
fn graded_pgvector_text(rank: usize) -> String {
    assert!(
        (1..=25).contains(&rank),
        "rank must be in 1..=25, got {rank}"
    );
    let mut parts = vec!["0".to_string(); 768];
    parts[0] = (26 - rank).to_string();
    parts[rank] = rank.to_string();
    format!("[{}]", parts.join(","))
}

async fn seed_chunk_with_graded_vector(pool: &sqlx::PgPool, user_id: &str, id: &str, rank: usize) {
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id, embedding)
         VALUES ($1, $1, 'content', 'note', $2, $3::text::vector)",
    )
    .bind(id)
    .bind(user_id)
    .bind(graded_pgvector_text(rank))
    .execute(pool)
    .await
    .unwrap();
}

/// Pins the `limit: 20` constant in the `"similar-to"` match arm
/// (`search/service.rs`'s `fubbik_db::repo::semantic::semantic_search(...,
/// 20)` call) — the same shape as
/// `tests/chunks_ai.rs::semantic_search_caps_limit_at_twenty`, but at the
/// unified-search clause layer rather than the standalone endpoint.
///
/// This matters specifically for `similar-to` because its resolved ids
/// are *intersected* with every other clause's ids
/// (`resolve_graph_clauses`' `intersect_ids`, `service.ts`'s `graphIds ?
/// graphIds.filter(...) : ids`): a silent regression from 20 down to,
/// say, 1 wouldn't just trim a semantic search's own result list — it
/// could collapse an otherwise-reasonable multi-clause search down to
/// almost nothing, with no error surfaced anywhere. 25 chunks are seeded,
/// each at a distinct, non-orthogonal, strictly-decreasing similarity to
/// the query vector (see `graded_pgvector_text`'s doc comment for why
/// orthogonal fillers would not do — tied distances prove nothing about
/// a specific cutoff), so "top 20 of 25 genuine candidates" is
/// unambiguous: exactly ranks 1..=20 must come back, never 21..=25.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn similar_to_clause_caps_resolved_ids_at_twenty(pool: sqlx::PgPool) {
    let server = ollama_mock(one_hot(0)).await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "similar-cap@b.test", "S").await;
    let user_id = user_id_for_email(&pool, "similar-cap@b.test").await;

    for rank in 1..=25 {
        seed_chunk_with_graded_vector(&pool, &user_id, &format!("g{rank}"), rank).await;
    }

    let res = post(
        app,
        "/api/search/query",
        &cookie,
        serde_json::json!({"clauses": [
            {"field": "similar-to", "operator": "is", "value": "authentication flow"}
        ]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;

    let ids: std::collections::HashSet<String> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        ids.len(),
        20,
        "exactly 20 of the 25 genuine (non-tied) candidates must resolve: {ids:?}"
    );
    for rank in 1..=20 {
        let id = format!("g{rank}");
        assert!(
            ids.contains(&id),
            "rank {rank} ({id}) is strictly better than every excluded rank and must be in the top 20: {ids:?}"
        );
    }
    for rank in 21..=25 {
        let id = format!("g{rank}");
        assert!(
            !ids.contains(&id),
            "rank {rank} ({id}) is strictly worse than 20 other genuine candidates and must not fit in the top 20: {ids:?}"
        );
    }
}
