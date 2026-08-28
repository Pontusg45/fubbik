//! Tests for `fubbik_api::context::{resolvers, service}` — the port of
//! `packages/api/src/context/resolvers.ts`.
//!
//! There is no HTTP surface for `/api/context/*` in this port yet (Task 5
//! only adds the resolver/enrichment layer; routes are a later task), so
//! these tests call `resolvers`/`service` functions directly against a
//! `PgPool` rather than driving an `axum::Router` through `oneshot`. Users
//! are seeded with `fubbik_db::repo::user::create` directly, the same
//! fixture pattern `crates/fubbik-db/tests/semantic.rs` uses, rather than
//! signing up over HTTP — there is no session boundary being tested here.

use fubbik_core::error::AppError;
use fubbik_db::repo::{chunk, chunk_meta, plan, requirement, user};

fn new_chunk(title: &str) -> chunk::NewChunk {
    chunk::NewChunk {
        title: title.to_string(),
        content: "x".repeat(200),
        chunk_type: "note".to_string(),
        rationale: None,
        alternatives: None,
        consequences: None,
        origin: "human".to_string(),
        review_status: "approved".to_string(),
        document_id: None,
        document_order: None,
    }
}

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "Test User", None)
        .await
        .unwrap()
        .id
}

// ---------------------------------------------------------------------------
// resolve_for_plan
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn resolve_for_plan_returns_chunks_linked_through_tasks(pool: sqlx::PgPool) {
    let user_id = seed_user(&pool, "plan-owner@b.test").await;

    let p = plan::create(&pool, &user_id, "A plan", None, None)
        .await
        .unwrap();
    let task = plan::create_task(
        &pool,
        &user_id,
        &p.id,
        "A task",
        None,
        serde_json::json!([]),
        None,
    )
    .await
    .unwrap()
    .unwrap();

    let linked_a = chunk::create(&pool, &user_id, new_chunk("Linked A"))
        .await
        .unwrap();
    let linked_b = chunk::create(&pool, &user_id, new_chunk("Linked B"))
        .await
        .unwrap();
    let unlinked = chunk::create(&pool, &user_id, new_chunk("Unlinked"))
        .await
        .unwrap();

    plan::add_task_chunk(&pool, &user_id, &p.id, &task.id, &linked_a.id, "context")
        .await
        .unwrap()
        .expect("chunk a should link");
    plan::add_task_chunk(&pool, &user_id, &p.id, &task.id, &linked_b.id, "created")
        .await
        .unwrap()
        .expect("chunk b should link");

    let ids = fubbik_api::context::resolvers::resolve_for_plan(&pool, &user_id, &p.id)
        .await
        .unwrap();

    assert!(
        ids.contains(&linked_a.id),
        "linked chunk A must be resolved: {ids:?}"
    );
    assert!(
        ids.contains(&linked_b.id),
        "linked chunk B must be resolved: {ids:?}"
    );
    assert!(
        !ids.contains(&unlinked.id),
        "an unlinked chunk must not be resolved: {ids:?}"
    );
}

/// Per the controller's ruling on this task: Node's `resolveForPlan(planId)`
/// takes no `user_id` and performs no ownership check at all (see
/// `resolvers.rs`'s module doc for the full citation trail) — this port
/// adds one, mirroring `plans::service::get_plan`'s `NotFound`-on-foreign-id
/// shape. This test pins the resolver's *own* check, not the downstream
/// chunk filter in `enrich_chunks`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn resolve_for_plan_is_scoped_to_the_owner(pool: sqlx::PgPool) {
    let owner_id = seed_user(&pool, "owner@b.test").await;
    let other_id = seed_user(&pool, "other@b.test").await;

    let p = plan::create(&pool, &owner_id, "Owner's plan", None, None)
        .await
        .unwrap();
    let task = plan::create_task(
        &pool,
        &owner_id,
        &p.id,
        "A task",
        None,
        serde_json::json!([]),
        None,
    )
    .await
    .unwrap()
    .unwrap();
    let linked = chunk::create(&pool, &owner_id, new_chunk("Owner's chunk"))
        .await
        .unwrap();
    plan::add_task_chunk(&pool, &owner_id, &p.id, &task.id, &linked.id, "context")
        .await
        .unwrap()
        .unwrap();

    // Half one: a foreign user resolving the plan gets NotFound, not an
    // empty candidate list and not someone else's data.
    let foreign_attempt =
        fubbik_api::context::resolvers::resolve_for_plan(&pool, &other_id, &p.id).await;
    assert!(
        matches!(foreign_attempt, Err(AppError::NotFound(_))),
        "a foreign user must get NotFound, got {foreign_attempt:?}"
    );

    // Half two: the plan itself is intact and the real owner can still
    // resolve it — proving the request was rejected, not the plan damaged.
    let owner_ids = fubbik_api::context::resolvers::resolve_for_plan(&pool, &owner_id, &p.id)
        .await
        .unwrap();
    assert!(
        owner_ids.contains(&linked.id),
        "the owner must still be able to resolve their own plan after a foreign attempt: {owner_ids:?}"
    );
}

/// Pins `resolve_for_plan`'s id order end to end, at the resolver itself —
/// not after `budget_and_format` has already received whatever order the
/// resolver handed it (that seam is covered separately in
/// `crates/fubbik-api/src/context/routes.rs`'s
/// `budget_and_format_preserves_enrichment_order_among_tied_scores`).
///
/// Node's dedup is `new Set<string>()` + `[...ids]`; JS `Set` iterates in
/// insertion order by spec, so Node's result order is exactly the order
/// its three sources are queried: analyze items, then requirement-linked
/// chunks, then task-linked chunks. This resolver ports that with a
/// `Vec`-plus-membership-`HashSet` (`push_unique` in `resolvers.rs`)
/// instead of collecting into a bare `HashSet` and calling
/// `.into_iter().collect()` — which would scramble the order via Rust's
/// per-construction-randomized default hasher (confirmed experimentally
/// while diagnosing this: five inserts into a fresh `HashSet`, printed
/// across five constructions in the same process, produced five different
/// orders).
///
/// This test cannot pass by the coincidence a same-order-across-two-calls
/// test risks: each source below contributes exactly one candidate id (two
/// analyze items, one requirement with one linked chunk, one task with one
/// linked chunk), so there is no per-source DB ordering ambiguity to
/// control for — the *only* thing that decides the returned sequence is
/// whether the resolver preserves first-encounter order across sources.
/// Asserted against a fully pinned expected sequence, not "did two calls
/// agree" — this repeats every call `RUNS` times specifically so a
/// regression back to a bare `HashSet` reliably shows up as a mismatch on
/// at least one iteration, rather than possibly matching the expected
/// order by chance.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn resolve_for_plan_preserves_first_encounter_order_across_all_three_sources(
    pool: sqlx::PgPool,
) {
    let user_id = seed_user(&pool, "order@b.test").await;
    let p = plan::create(&pool, &user_id, "Ordered plan", None, None)
        .await
        .unwrap();

    // Source 1: two chunk-kind analyze items, queried `kind ASC, "order"
    // ASC, id ASC` — same kind, so insertion order decides `"order"`.
    let analyze_a = chunk::create(&pool, &user_id, new_chunk("Analyze A"))
        .await
        .unwrap();
    let analyze_b = chunk::create(&pool, &user_id, new_chunk("Analyze B"))
        .await
        .unwrap();
    plan::create_analyze_item(
        &pool,
        &user_id,
        &p.id,
        "chunk",
        Some(&analyze_a.id),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    plan::create_analyze_item(
        &pool,
        &user_id,
        &p.id,
        "chunk",
        Some(&analyze_b.id),
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // Source 2: one requirement, linked to exactly one chunk — a single
    // linked chunk sidesteps `requirement::get_chunks`'s documented lack
    // of an `ORDER BY` (fine when there's only one row to return).
    let req_chunk = chunk::create(&pool, &user_id, new_chunk("Requirement chunk"))
        .await
        .unwrap();
    let req = requirement::create(
        &pool,
        &user_id,
        fubbik_db::repo::requirement::NewRequirement {
            title: "Req".to_string(),
            description: None,
            steps: vec![],
            priority: None,
            space_id: None,
            use_case_id: None,
            origin: "human".to_string(),
            review_status: "approved".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("requirement should be created");
    plan::add_requirement(&pool, &user_id, &p.id, &req.id)
        .await
        .unwrap()
        .expect("requirement should link to the plan");
    requirement::set_chunks(
        &pool,
        &user_id,
        &req.id,
        std::slice::from_ref(&req_chunk.id),
    )
    .await
    .unwrap();

    // Source 3: one task, linked to exactly one chunk.
    let task_chunk = chunk::create(&pool, &user_id, new_chunk("Task chunk"))
        .await
        .unwrap();
    let task = plan::create_task(
        &pool,
        &user_id,
        &p.id,
        "A task",
        None,
        serde_json::json!([]),
        None,
    )
    .await
    .unwrap()
    .unwrap();
    plan::add_task_chunk(&pool, &user_id, &p.id, &task.id, &task_chunk.id, "context")
        .await
        .unwrap()
        .expect("chunk should link to the task");

    let expected = vec![
        analyze_a.id.clone(),
        analyze_b.id.clone(),
        req_chunk.id.clone(),
        task_chunk.id.clone(),
    ];

    const RUNS: usize = 5;
    for run in 0..RUNS {
        let ids = fubbik_api::context::resolvers::resolve_for_plan(&pool, &user_id, &p.id)
            .await
            .unwrap();
        assert_eq!(
            ids, expected,
            "run {run}: resolve_for_plan must return ids in first-encounter order \
             (analyze items, then requirement chunks, then task chunks), got {ids:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// resolve_for_concept
// ---------------------------------------------------------------------------

fn one_hot(index: usize) -> String {
    let mut parts = vec!["0"; 768];
    parts[index] = "1";
    format!("[{}]", parts.join(","))
}

async fn seed_chunk_with_vector(
    pool: &sqlx::PgPool,
    user_id: &str,
    id: &str,
    title: &str,
    hot: usize,
) {
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id, embedding) \
         VALUES ($1, $2, $3, 'note', $4, $5::text::vector)",
    )
    .bind(id)
    .bind(title)
    .bind("x".repeat(200))
    .bind(user_id)
    .bind(one_hot(hot))
    .execute(pool)
    .await
    .unwrap();
}

async fn ollama_embeddings_mock(vector: Vec<f32>) -> wiremock::MockServer {
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
async fn resolve_for_concept_combines_semantic_and_text_matches(pool: sqlx::PgPool) {
    let user_id = seed_user(&pool, "concept@b.test").await;

    // A chunk whose embedding is near the mocked query embedding, but whose
    // title/content share nothing with the query text.
    seed_chunk_with_vector(
        &pool,
        &user_id,
        "semantic-hit",
        "Something Else Entirely",
        0,
    )
    .await;

    // A chunk that only matches on title text, with no embedding at all —
    // it can only be found by the text-search half.
    let title_match = chunk::create(&pool, &user_id, new_chunk("Widget Assembly Guide"))
        .await
        .unwrap();

    let server = ollama_embeddings_mock(
        vec![{
            let mut v = vec![0.0f32; 768];
            v[0] = 1.0;
            v
        }][0]
            .clone(),
    )
    .await;
    let ai = fubbik_ai::OllamaClient::new(server.uri());

    let ids =
        fubbik_api::context::resolvers::resolve_for_concept(&pool, &ai, &user_id, "widget", None)
            .await
            .unwrap();

    assert!(
        ids.contains(&"semantic-hit".to_string()),
        "the semantically-near chunk must appear: {ids:?}"
    );
    assert!(
        ids.contains(&title_match.id),
        "the title-matching chunk must appear: {ids:?}"
    );
}

// ---------------------------------------------------------------------------
// resolve_for_files
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn resolve_for_files_matches_file_refs_and_applies_to_globs(pool: sqlx::PgPool) {
    let user_id = seed_user(&pool, "files@b.test").await;

    let file_ref_chunk = chunk::create(&pool, &user_id, new_chunk("File-ref chunk"))
        .await
        .unwrap();
    chunk_meta::replace_file_refs(
        &pool,
        &file_ref_chunk.id,
        &user_id,
        &[chunk_meta::FileRefInput::from("src/foo.rs")],
    )
    .await
    .unwrap();

    let applies_to_chunk = chunk::create(&pool, &user_id, new_chunk("Applies-to chunk"))
        .await
        .unwrap();
    chunk_meta::replace_applies_to(
        &pool,
        &applies_to_chunk.id,
        &user_id,
        &[chunk_meta::AppliesToInput::from("src/**/*.rs")],
    )
    .await
    .unwrap();

    let unrelated = chunk::create(&pool, &user_id, new_chunk("Unrelated chunk"))
        .await
        .unwrap();

    let paths = vec!["src/foo.rs".to_string(), "src/nested/bar.rs".to_string()];
    let ai = fubbik_ai::OllamaClient::new("http://127.0.0.1:1");
    let ids = fubbik_api::context::resolvers::resolve_for_files(&pool, &ai, &user_id, &paths, None)
        .await
        .unwrap();

    assert!(
        ids.contains(&file_ref_chunk.id),
        "file-ref match must be resolved: {ids:?}"
    );
    assert!(
        ids.contains(&applies_to_chunk.id),
        "applies-to glob match must be resolved: {ids:?}"
    );
    assert!(
        !ids.contains(&unrelated.id),
        "an unrelated chunk must not be resolved: {ids:?}"
    );
}

// ---------------------------------------------------------------------------
// enrich_chunks
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_chunks_computes_health_and_flags_stale(pool: sqlx::PgPool) {
    let user_id = seed_user(&pool, "enrich@b.test").await;
    let c = chunk::create(&pool, &user_id, new_chunk("Stale chunk"))
        .await
        .unwrap();

    sqlx::query("INSERT INTO chunk_staleness (id, chunk_id, reason) VALUES ($1, $2, 'age')")
        .bind(fubbik_db::new_id())
        .bind(&c.id)
        .execute(&pool)
        .await
        .unwrap();

    let ids = vec![c.id.clone()];
    let enriched = fubbik_api::context::service::enrich_chunks(&pool, &user_id, &ids)
        .await
        .unwrap();

    assert_eq!(enriched.len(), 1);
    let meta = &enriched[0];
    assert!(
        meta.is_stale,
        "a chunk with an undismissed staleness flag must be flagged is_stale"
    );
    assert!(
        (0..=100).contains(&meta.health_score),
        "health score must be within 0..=100, got {}",
        meta.health_score
    );
}

// ---------------------------------------------------------------------------
// mutation-test scaffolding note (not a test): Step 5 of the task brief
// asks for the ownership check in `resolve_for_plan` to be temporarily
// removed and `resolve_for_plan_is_scoped_to_the_owner` re-run to confirm
// it fails. That is done by hand against the working tree and reverted —
// see the task report for the captured output.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// HTTP routes: `/api/context/for-plan`, `/api/context/about`,
// `/api/context/for-files` — Task 6.
// ---------------------------------------------------------------------------

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

async fn user_id_for_email(pool: &sqlx::PgPool, email: &str) -> String {
    sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = $1"#, email)
        .fetch_one(pool)
        .await
        .unwrap()
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

/// `maxTokens=50` excludes a large linked chunk that `maxTokens=50000`
/// includes — pinning `budget_chunks` actually receiving the parsed value,
/// not just that both requests return 200 (Step 5's mutation target).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn for_plan_returns_the_plans_chunks_within_budget(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "plan-budget@b.test", "Plan Budget").await;
    let user_id = user_id_for_email(&pool, "plan-budget@b.test").await;

    let p = plan::create(&pool, &user_id, "Budget plan", None, None)
        .await
        .unwrap();
    let task = plan::create_task(
        &pool,
        &user_id,
        &p.id,
        "A task",
        None,
        serde_json::json!([]),
        None,
    )
    .await
    .unwrap()
    .unwrap();

    // ~650+ tokens of content — comfortably over a 50-token budget, and
    // comfortably under a 50000-token one.
    let mut big_chunk = new_chunk("Large Linked Chunk");
    big_chunk.content = "word ".repeat(500);
    let linked = chunk::create(&pool, &user_id, big_chunk).await.unwrap();
    plan::add_task_chunk(&pool, &user_id, &p.id, &task.id, &linked.id, "context")
        .await
        .unwrap()
        .expect("chunk should link to the task");

    let small = get(
        app.clone(),
        &cookie,
        &format!("/api/context/for-plan?planId={}&maxTokens=50", p.id),
    )
    .await;
    assert_eq!(small.status(), StatusCode::OK);
    let small_body = json_body(small).await;
    let small_total = small_body["totalChunks"].as_u64().unwrap();

    let large = get(
        app.clone(),
        &cookie,
        &format!("/api/context/for-plan?planId={}&maxTokens=50000", p.id),
    )
    .await;
    assert_eq!(large.status(), StatusCode::OK);
    let large_body = json_body(large).await;
    let large_total = large_body["totalChunks"].as_u64().unwrap();
    let large_content = large_body["content"].as_str().unwrap();

    assert!(
        large_content.contains("Large Linked Chunk"),
        "the plan's linked chunk must appear when the budget is generous: {large_content}"
    );
    assert!(
        small_total < large_total,
        "maxTokens=50 must yield strictly fewer chunks than maxTokens=50000: small={small_total} large={large_total}"
    );
}

/// `resolve_for_plan` 404s a cross-user request rather than returning an
/// empty context — the deliberate tightening over Node documented on the
/// resolver itself. This pins that behaviour through the HTTP route.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn for_plan_404s_for_another_users_plan(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let _owner_cookie = signup(app.clone(), "plan-owner-http@b.test", "Owner").await;
    let owner_id = user_id_for_email(&pool, "plan-owner-http@b.test").await;
    let intruder_cookie = signup(app.clone(), "plan-intruder-http@b.test", "Intruder").await;

    let p = plan::create(&pool, &owner_id, "Private plan", None, None)
        .await
        .unwrap();

    let res = get(
        app.clone(),
        &intruder_cookie,
        &format!("/api/context/for-plan?planId={}", p.id),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// The `/api/context/about` route over a wiremock `/api/embeddings`:
/// asserts the semantically-near chunk appears and a chunk that matches
/// neither the embedding nor the search text does not.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn about_finds_a_chunk_by_concept(pool: sqlx::PgPool) {
    let server = ollama_embeddings_mock({
        let mut v = vec![0.0f32; 768];
        v[0] = 1.0;
        v
    })
    .await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "about-http@b.test", "About").await;
    let user_id = user_id_for_email(&pool, "about-http@b.test").await;

    // Near the mocked query embedding (hot index 0).
    seed_chunk_with_vector(&pool, &user_id, "near-chunk", "Widget Manual", 0).await;
    // No embedding at all (excluded from the semantic path) and shares no
    // text with the query, so the text-search path can't find it either.
    chunk::create(&pool, &user_id, new_chunk("Completely Unrelated Topic"))
        .await
        .unwrap();

    let res = get(
        app.clone(),
        &cookie,
        "/api/context/about?q=widget&maxTokens=50000",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let content = body["content"].as_str().unwrap();

    assert!(
        content.contains("Widget Manual"),
        "the semantically-near chunk must appear: {content}"
    );
    assert!(
        !content.contains("Completely Unrelated Topic"),
        "a distant, non-matching chunk must not appear: {content}"
    );
}

/// `paths=a,b` resolves chunks for both paths, not just the first.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn for_files_accepts_a_csv_of_paths(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "files-http@b.test", "Files").await;
    let user_id = user_id_for_email(&pool, "files-http@b.test").await;

    let a = chunk::create(&pool, &user_id, new_chunk("A File Chunk"))
        .await
        .unwrap();
    chunk_meta::replace_file_refs(
        &pool,
        &a.id,
        &user_id,
        &[chunk_meta::FileRefInput::from("src/a.rs")],
    )
    .await
    .unwrap();

    let b = chunk::create(&pool, &user_id, new_chunk("B File Chunk"))
        .await
        .unwrap();
    chunk_meta::replace_file_refs(
        &pool,
        &b.id,
        &user_id,
        &[chunk_meta::FileRefInput::from("src/b.rs")],
    )
    .await
    .unwrap();

    let res = get(
        app.clone(),
        &cookie,
        "/api/context/for-files?paths=src/a.rs,src/b.rs&maxTokens=50000",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let content = body["content"].as_str().unwrap();

    assert!(
        content.contains("A File Chunk"),
        "the chunk for the first path must be returned: {content}"
    );
    assert!(
        content.contains("B File Chunk"),
        "the chunk for the second path must be returned: {content}"
    );
}

/// `format` defaults to `structured-md` (a `content` string, no `sections`)
/// and `structured-json` is selectable (`sections`, no `content`) — the two
/// bodies must genuinely differ, not just both return 200.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn format_defaults_to_structured_md_and_json_is_selectable(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "format-http@b.test", "Format").await;
    let user_id = user_id_for_email(&pool, "format-http@b.test").await;

    let c = chunk::create(&pool, &user_id, new_chunk("Formatted Chunk"))
        .await
        .unwrap();
    chunk_meta::replace_file_refs(
        &pool,
        &c.id,
        &user_id,
        &[chunk_meta::FileRefInput::from("src/formatted.rs")],
    )
    .await
    .unwrap();

    let default_res = get(
        app.clone(),
        &cookie,
        "/api/context/for-files?paths=src/formatted.rs&maxTokens=50000",
    )
    .await;
    assert_eq!(default_res.status(), StatusCode::OK);
    let default_body = json_body(default_res).await;
    assert_eq!(default_body["format"], "structured-md");
    assert!(
        default_body.get("content").is_some(),
        "the default format must carry a markdown `content` string: {default_body:?}"
    );
    assert!(
        default_body.get("sections").is_none(),
        "the default format must not carry `sections`: {default_body:?}"
    );

    let json_res = get(
        app.clone(),
        &cookie,
        "/api/context/for-files?paths=src/formatted.rs&maxTokens=50000&format=structured-json",
    )
    .await;
    assert_eq!(json_res.status(), StatusCode::OK);
    let json_body_val = json_body(json_res).await;
    assert_eq!(json_body_val["format"], "structured-json");
    assert!(
        json_body_val.get("sections").is_some(),
        "the json format must carry `sections`: {json_body_val:?}"
    );
    assert!(
        json_body_val.get("content").is_none(),
        "the json format must not carry a markdown `content` string: {json_body_val:?}"
    );

    assert_ne!(
        serde_json::to_string(&default_body).unwrap(),
        serde_json::to_string(&json_body_val).unwrap(),
        "the two formats must produce genuinely different response bodies"
    );
}

// ---------------------------------------------------------------------------
// Context snapshots — `POST /api/context/snapshot`,
// `GET /api/context/snapshot/{id}`, `GET /api/context/snapshots`,
// `DELETE /api/context/snapshot/{id}`.
//
// Every one of these four tests drives a snapshot through a plan
// (`{"planId": ...}`), the same resolver `resolve_for_plan_*` above
// already covers end to end — snapshot creation is not re-testing the
// resolver, only that its output survives being frozen into JSONB and
// read back through the four routes, scoped to the caller throughout.

/// Creates a plan owned by `user_id` with one task linked to one chunk,
/// and returns `(plan_id, chunk_id, chunk_title)` — the fixture every
/// snapshot test below builds a `{"planId": ...}` snapshot from.
async fn seed_plan_with_chunk(pool: &sqlx::PgPool, user_id: &str, title: &str) -> (String, String) {
    let p = plan::create(pool, user_id, "Snapshot plan", None, None)
        .await
        .unwrap();
    let task = plan::create_task(
        pool,
        user_id,
        &p.id,
        "A task",
        None,
        serde_json::json!([]),
        None,
    )
    .await
    .unwrap()
    .unwrap();
    let c = chunk::create(pool, user_id, new_chunk(title))
        .await
        .unwrap();
    plan::add_task_chunk(pool, user_id, &p.id, &task.id, &c.id, "context")
        .await
        .unwrap()
        .expect("chunk should link to the task");
    (p.id, c.id)
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn snapshot_round_trips_its_frozen_content(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "snap-roundtrip@b.test", "Snap").await;
    let user_id = user_id_for_email(&pool, "snap-roundtrip@b.test").await;

    let (plan_id, _chunk_id) =
        seed_plan_with_chunk(&pool, &user_id, "Snapshot Round Trip Chunk").await;

    let create_res = post(
        app.clone(),
        &cookie,
        "/api/context/snapshot",
        serde_json::json!({ "planId": plan_id, "maxTokens": 50000 }),
    )
    .await;
    assert_eq!(create_res.status(), StatusCode::OK, "create must succeed");
    let create_body = json_body(create_res).await;
    let snapshot_id = create_body["snapshotId"]
        .as_str()
        .expect("create must return a snapshotId")
        .to_string();
    assert_eq!(
        create_body["chunkCount"], 1,
        "the plan's single linked chunk must be resolved and budgeted: {create_body:?}"
    );

    let get_res = get(
        app.clone(),
        &cookie,
        &format!("/api/context/snapshot/{snapshot_id}"),
    )
    .await;
    assert_eq!(get_res.status(), StatusCode::OK);
    let get_body = json_body(get_res).await;

    assert_eq!(get_body["id"], snapshot_id);
    assert_eq!(
        get_body["tokenCount"], create_body["tokenCount"],
        "the retrieved snapshot's token count must match what create reported: {get_body:?}"
    );
    let chunks = get_body["chunks"]
        .as_array()
        .expect("retrieved snapshot must carry its frozen chunks array");
    assert_eq!(chunks.len(), 1);
    assert_eq!(
        chunks[0]["title"], "Snapshot Round Trip Chunk",
        "the frozen chunk content must match what was resolved at create time: {chunks:?}"
    );
    assert_eq!(get_body["query"]["planId"], plan_id);
}

/// Both halves: user B retrieving user A's snapshot gets 404, and A's
/// snapshot is still retrievable by A afterwards — proving the request was
/// rejected, not the row damaged.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn snapshot_retrieval_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie_a = signup(app.clone(), "snap-scope-a@b.test", "Alice").await;
    let user_a = user_id_for_email(&pool, "snap-scope-a@b.test").await;
    let cookie_b = signup(app.clone(), "snap-scope-b@b.test", "Bob").await;

    let (plan_id, _chunk_id) = seed_plan_with_chunk(&pool, &user_a, "Alice's Scoped Chunk").await;

    let create_body = json_body(
        post(
            app.clone(),
            &cookie_a,
            "/api/context/snapshot",
            serde_json::json!({ "planId": plan_id }),
        )
        .await,
    )
    .await;
    let snapshot_id = create_body["snapshotId"].as_str().unwrap().to_string();

    // Half one: a foreign user gets 404, not Alice's frozen content.
    let foreign_res = get(
        app.clone(),
        &cookie_b,
        &format!("/api/context/snapshot/{snapshot_id}"),
    )
    .await;
    assert_eq!(
        foreign_res.status(),
        StatusCode::NOT_FOUND,
        "a foreign user must not be able to retrieve another user's snapshot"
    );

    // Half two: the snapshot is intact and Alice can still read it — proof
    // the request was rejected, not that the row was damaged.
    let owner_res = get(
        app.clone(),
        &cookie_a,
        &format!("/api/context/snapshot/{snapshot_id}"),
    )
    .await;
    assert_eq!(owner_res.status(), StatusCode::OK);
    let owner_body = json_body(owner_res).await;
    assert_eq!(owner_body["id"], snapshot_id);
    assert_eq!(owner_body["chunks"][0]["title"], "Alice's Scoped Chunk");
}

/// Both halves: user B deleting user A's snapshot gets 404, and A's
/// snapshot survives (both a GET and a subsequent delete by A itself
/// succeed) — again proving rejection, not silent damage.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn snapshot_deletion_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie_a = signup(app.clone(), "snap-del-a@b.test", "Alice").await;
    let user_a = user_id_for_email(&pool, "snap-del-a@b.test").await;
    let cookie_b = signup(app.clone(), "snap-del-b@b.test", "Bob").await;

    let (plan_id, _chunk_id) =
        seed_plan_with_chunk(&pool, &user_a, "Alice's Deletable Chunk").await;

    let create_body = json_body(
        post(
            app.clone(),
            &cookie_a,
            "/api/context/snapshot",
            serde_json::json!({ "planId": plan_id }),
        )
        .await,
    )
    .await;
    let snapshot_id = create_body["snapshotId"].as_str().unwrap().to_string();

    // Half one: a foreign delete is rejected with 404.
    let foreign_delete = delete(
        app.clone(),
        &cookie_b,
        &format!("/api/context/snapshot/{snapshot_id}"),
    )
    .await;
    assert_eq!(
        foreign_delete.status(),
        StatusCode::NOT_FOUND,
        "a foreign user must not be able to delete another user's snapshot"
    );

    // Half two: the snapshot survived the foreign attempt — the owner can
    // still read it, and can still delete it herself afterwards.
    let survives = get(
        app.clone(),
        &cookie_a,
        &format!("/api/context/snapshot/{snapshot_id}"),
    )
    .await;
    assert_eq!(
        survives.status(),
        StatusCode::OK,
        "the snapshot must survive a rejected foreign delete attempt"
    );

    let owner_delete = delete(
        app.clone(),
        &cookie_a,
        &format!("/api/context/snapshot/{snapshot_id}"),
    )
    .await;
    assert_eq!(
        owner_delete.status(),
        StatusCode::OK,
        "the real owner must still be able to delete her own snapshot afterwards"
    );

    let gone = get(
        app.clone(),
        &cookie_a,
        &format!("/api/context/snapshot/{snapshot_id}"),
    )
    .await;
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
}

/// `GET /api/context/snapshots` must return only the caller's own
/// snapshots — not a foreign user's, even though both exist in the table.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn snapshots_list_only_returns_the_callers_own(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie_a = signup(app.clone(), "snap-list-a@b.test", "Alice").await;
    let user_a = user_id_for_email(&pool, "snap-list-a@b.test").await;
    let cookie_b = signup(app.clone(), "snap-list-b@b.test", "Bob").await;
    let user_b = user_id_for_email(&pool, "snap-list-b@b.test").await;

    let (plan_a, _) = seed_plan_with_chunk(&pool, &user_a, "Alice's List Chunk").await;
    let (plan_b, _) = seed_plan_with_chunk(&pool, &user_b, "Bob's List Chunk").await;

    let snap_a = json_body(
        post(
            app.clone(),
            &cookie_a,
            "/api/context/snapshot",
            serde_json::json!({ "planId": plan_a }),
        )
        .await,
    )
    .await;
    let snap_a_id = snap_a["snapshotId"].as_str().unwrap().to_string();

    let snap_b = json_body(
        post(
            app.clone(),
            &cookie_b,
            "/api/context/snapshot",
            serde_json::json!({ "planId": plan_b }),
        )
        .await,
    )
    .await;
    let snap_b_id = snap_b["snapshotId"].as_str().unwrap().to_string();

    let list_res = get(app.clone(), &cookie_a, "/api/context/snapshots").await;
    assert_eq!(list_res.status(), StatusCode::OK);
    let list_body = json_body(list_res).await;
    let ids: Vec<&str> = list_body
        .as_array()
        .expect("list must be a bare array")
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();

    assert!(
        ids.contains(&snap_a_id.as_str()),
        "Alice's own snapshot must appear in her list: {ids:?}"
    );
    assert!(
        !ids.contains(&snap_b_id.as_str()),
        "Bob's snapshot must never appear in Alice's list: {ids:?}"
    );
}
