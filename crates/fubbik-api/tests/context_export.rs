//! Tests for `GET /api/chunks/export/context` and
//! `GET /api/chunks/export/claude-md` — the port of
//! `packages/api/src/context-export/{service,claude-md}.ts` (Task 9).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use fubbik_db::repo::requirement::{RequirementStep, StepKeyword};
use fubbik_db::repo::{chunk, chunk_meta, plan, requirement, tag};
use http_body_util::BodyExt;
use tower::ServiceExt;

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

// ---------------------------------------------------------------------------
// GET /api/chunks/export/context
// ---------------------------------------------------------------------------

/// `maxTokens=50` must exclude the large chunks a `maxTokens=50000` budget
/// admits — pinning `export_context` actually threading the parsed
/// `maxTokens` value into `budget_metadata`, not just returning 200 either
/// way (Step 5's mutation target for the sibling CLAUDE.md endpoint; this
/// is the equivalent pin for the export/context endpoint).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn export_context_respects_max_tokens(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "export-budget@b.test", "Export Budget").await;
    let user_id = user_id_for_email(&pool, "export-budget@b.test").await;

    // Three ~650-token chunks: comfortably over a 50-token budget,
    // comfortably under a 50000-token one.
    for i in 0..3 {
        let mut c = new_chunk(&format!("Large Chunk {i}"));
        c.content = "word ".repeat(500);
        chunk::create(&pool, &user_id, c).await.unwrap();
    }

    let small = get(
        app.clone(),
        &cookie,
        "/api/chunks/export/context?maxTokens=50&format=json",
    )
    .await;
    assert_eq!(small.status(), StatusCode::OK);
    let small_body = json_body(small).await;
    let small_chunks = small_body["chunks"].as_array().unwrap().len();

    let large = get(
        app.clone(),
        &cookie,
        "/api/chunks/export/context?maxTokens=50000&format=json",
    )
    .await;
    assert_eq!(large.status(), StatusCode::OK);
    let large_body = json_body(large).await;
    let large_chunks = large_body["chunks"].as_array().unwrap().len();

    assert!(
        small_chunks < large_chunks,
        "maxTokens=50 must yield strictly fewer chunks than maxTokens=50000: \
         small={small_chunks} large={large_chunks}"
    );
}

/// `forPath` must be able to flip which of two same-sized chunks survives a
/// budget that only fits one. `chunk_other` intrinsically outscores
/// `chunk_referenced` (type=document +3 and a rationale +2, vs. type=note
/// +1 and no rationale, both otherwise identical) — so a control request
/// with no `forPath` picks `chunk_other`. Setting `forPath` to a path only
/// `chunk_referenced` is linked to must add enough of a bonus (+15) to flip
/// that outcome. Asserting only that both chunks appear somewhere in a
/// generous budget would not catch a zeroed-out (or merely present but
/// too-small) bonus; forcing a one-survivor budget makes the flip the only
/// way this test can pass.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn export_context_boosts_chunks_relevant_to_for_path(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "export-forpath@b.test", "Export ForPath").await;
    let user_id = user_id_for_email(&pool, "export-forpath@b.test").await;

    let mut referenced = new_chunk("Chunk Referenced");
    referenced.content = "word ".repeat(500);
    referenced.chunk_type = "note".to_string();
    referenced.review_status = "draft".to_string();
    let referenced = chunk::create(&pool, &user_id, referenced).await.unwrap();
    chunk_meta::replace_file_refs(
        &pool,
        &referenced.id,
        &user_id,
        &[chunk_meta::FileRefInput::from("src/target.rs")],
    )
    .await
    .unwrap();

    let mut other = new_chunk("Chunk Other");
    other.content = "word ".repeat(500);
    other.chunk_type = "document".to_string();
    other.rationale = Some("because reasons".to_string());
    other.review_status = "draft".to_string();
    chunk::create(&pool, &user_id, other).await.unwrap();

    // Control: with no `forPath`, the intrinsically higher-scored chunk
    // (`Chunk Other`) wins a budget that fits exactly one ~650-token chunk.
    let control = get(
        app.clone(),
        &cookie,
        "/api/chunks/export/context?maxTokens=700&format=json",
    )
    .await;
    assert_eq!(control.status(), StatusCode::OK);
    let control_body = json_body(control).await;
    let control_chunks = control_body["chunks"].as_array().unwrap();
    assert_eq!(
        control_chunks.len(),
        1,
        "the 700-token budget must admit exactly one chunk"
    );
    assert_eq!(
        control_chunks[0]["title"], "Chunk Other",
        "without forPath, the intrinsically higher-scored chunk must win"
    );

    // With `forPath` set to the path only `Chunk Referenced` is linked to,
    // the +15 bonus must flip the winner.
    let boosted = get(
        app.clone(),
        &cookie,
        "/api/chunks/export/context?maxTokens=700&format=json&forPath=src/target.rs",
    )
    .await;
    assert_eq!(boosted.status(), StatusCode::OK);
    let boosted_body = json_body(boosted).await;
    let boosted_chunks = boosted_body["chunks"].as_array().unwrap();
    assert_eq!(
        boosted_chunks.len(),
        1,
        "the 700-token budget must still admit exactly one chunk"
    );
    assert_eq!(
        boosted_chunks[0]["title"], "Chunk Referenced",
        "the forPath bonus must outrank the chunk with no file-ref relevance"
    );
}

// ---------------------------------------------------------------------------
// GET /api/chunks/export/claude-md
// ---------------------------------------------------------------------------

/// Only a chunk tagged `claude-context` (the default tag) appears in the
/// generated document; an untagged chunk owned by the same user must not.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn claude_md_includes_tagged_chunks_and_excludes_untagged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "claude-md-tags@b.test", "Claude Tags").await;
    let user_id = user_id_for_email(&pool, "claude-md-tags@b.test").await;

    let tagged = chunk::create(&pool, &user_id, new_chunk("Documented Convention"))
        .await
        .unwrap();
    let t = tag::find_or_create(&pool, &user_id, "claude-context")
        .await
        .unwrap();
    tag::set_chunk_tags(&pool, &user_id, &tagged.id, &[t.id])
        .await
        .unwrap();

    chunk::create(&pool, &user_id, new_chunk("Undocumented Secret"))
        .await
        .unwrap();

    let res = get(app.clone(), &cookie, "/api/chunks/export/claude-md").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let content = body["content"].as_str().unwrap();

    assert_eq!(body["chunks"].as_u64().unwrap(), 1);
    assert!(
        content.contains("Documented Convention"),
        "tagged chunk must appear: {content}"
    );
    assert!(
        !content.contains("Undocumented Secret"),
        "untagged chunk must not appear: {content}"
    );
}

/// A single tagged chunk whose content alone comfortably exceeds 32000
/// tokens must be visibly truncated when `maxTokens` is omitted, but must
/// appear in full when an explicit, generous `maxTokens` is given — pinning
/// that the 32000 default is actually applied, not merely accepted as a
/// present-but-unused query param.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn claude_md_defaults_to_a_32000_token_budget(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "claude-md-budget@b.test", "Claude Budget").await;
    let user_id = user_id_for_email(&pool, "claude-md-budget@b.test").await;

    let mut giant = new_chunk("Giant Chunk");
    // ~40000 words -> comfortably more than 32000 tokens under o200k_base,
    // where this content's short, whitespace-separated words tokenize at
    // roughly one token apiece.
    giant.content = "word ".repeat(40_000);
    let giant = chunk::create(&pool, &user_id, giant).await.unwrap();
    let t = tag::find_or_create(&pool, &user_id, "claude-context")
        .await
        .unwrap();
    tag::set_chunk_tags(&pool, &user_id, &giant.id, &[t.id])
        .await
        .unwrap();

    let default_res = get(app.clone(), &cookie, "/api/chunks/export/claude-md").await;
    assert_eq!(default_res.status(), StatusCode::OK);
    let default_body = json_body(default_res).await;
    let default_content = default_body["content"].as_str().unwrap();

    let full_res = get(
        app.clone(),
        &cookie,
        "/api/chunks/export/claude-md?maxTokens=1000000",
    )
    .await;
    assert_eq!(full_res.status(), StatusCode::OK);
    let full_body = json_body(full_res).await;
    let full_content = full_body["content"].as_str().unwrap();

    assert!(
        full_content.contains("### Giant Chunk"),
        "a 1,000,000-token budget must include the giant chunk in full: {full_content}"
    );
    assert!(
        !default_content.contains("### Giant Chunk"),
        "the default (omitted maxTokens) budget must truncate the giant chunk out: {default_content}"
    );
    assert!(
        default_content.contains("Truncated:"),
        "the default budget must record the truncation: {default_content}"
    );
}

/// The document includes both an in-progress plan (with its task
/// completion ratio and pending-task checklist) and a requirement (with its
/// BDD steps and linked chunks).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn claude_md_includes_active_plans_and_requirements(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "claude-md-plans@b.test", "Claude Plans").await;
    let user_id = user_id_for_email(&pool, "claude-md-plans@b.test").await;

    // An in-progress plan with one done task and one pending task.
    let p = plan::create(&pool, &user_id, "Ship the export endpoints", None, None)
        .await
        .unwrap();
    plan::update(&pool, &user_id, &p.id, None, None, Some("in_progress"))
        .await
        .unwrap();

    let done_task = plan::create_task(
        &pool,
        &user_id,
        &p.id,
        "Write the service",
        None,
        serde_json::json!([]),
        None,
    )
    .await
    .unwrap()
    .unwrap();
    plan::mark_task_done_and_unblock(&pool, &user_id, &p.id, &done_task.id)
        .await
        .unwrap();

    plan::create_task(
        &pool,
        &user_id,
        &p.id,
        "Write the tests",
        None,
        serde_json::json!([]),
        None,
    )
    .await
    .unwrap()
    .unwrap();

    // A requirement with steps and a linked chunk.
    let linked_chunk = chunk::create(&pool, &user_id, new_chunk("Linked Chunk"))
        .await
        .unwrap();
    let req = requirement::create(
        &pool,
        &user_id,
        requirement::NewRequirement {
            title: "Exports respect a token budget".to_string(),
            description: None,
            steps: vec![
                RequirementStep {
                    keyword: StepKeyword::Given,
                    text: "a small maxTokens".to_string(),
                    params: None,
                },
                RequirementStep {
                    keyword: StepKeyword::Then,
                    text: "fewer chunks are returned".to_string(),
                    params: None,
                },
            ],
            priority: None,
            space_id: None,
            use_case_id: None,
            origin: "human".to_string(),
            review_status: "draft".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    requirement::set_chunks(
        &pool,
        &user_id,
        &req.id,
        std::slice::from_ref(&linked_chunk.id),
    )
    .await
    .unwrap();

    let res = get(app.clone(), &cookie, "/api/chunks/export/claude-md").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let content = body["content"].as_str().unwrap();

    assert!(
        content.contains("## Active Plans"),
        "must have an Active Plans section: {content}"
    );
    assert!(
        content.contains("Ship the export endpoints (1/2 tasks — 50%)"),
        "must show the plan's task completion ratio: {content}"
    );
    assert!(
        content.contains("- [ ] Write the tests"),
        "must list the still-pending task: {content}"
    );
    assert!(
        !content.contains("Write the service"),
        "a done task must not appear in the pending checklist: {content}"
    );

    assert!(
        content.contains("## Requirements"),
        "must have a Requirements section: {content}"
    );
    assert!(
        content.contains("Exports respect a token budget"),
        "must show the requirement's title: {content}"
    );
    assert!(
        content.contains("**given** a small maxTokens"),
        "must render the requirement's BDD steps: {content}"
    );
    assert!(
        content.contains("**Linked chunks:** Linked Chunk"),
        "must list the requirement's linked chunks: {content}"
    );
}

// ---------------------------------------------------------------------------
// Review follow-up: both endpoints must see beyond `chunk::list`'s
// HTTP-facing 100-row clamp (`chunk::list_internal`, `crates/fubbik-db/src/
// repo/chunk.rs`). Both tests below bulk-insert their filler rows via a
// single SQL statement each (not 150+ HTTP calls, and not 150+ `chunk::
// create` round trips) specifically to keep this fast — the whole point is
// to seed "a lot of rows" cheaply, not to exercise chunk creation itself.
// ---------------------------------------------------------------------------

/// `export_context`'s two `chunk::list_internal` calls (`service.rs`,
/// `FETCH_LIMIT = 500`) must see a chunk ranked 151st, which a 100-row cap
/// would silently drop before enrichment/scoring/budgeting ever see it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn export_context_considers_more_than_100_qualifying_chunks(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "export-wide@b.test", "Export Wide").await;
    let user_id = user_id_for_email(&pool, "export-wide@b.test").await;

    // 150 filler chunks, ids "filler-0001".."filler-0150", all approved
    // (the column default), all inserted — and therefore `created_at`
    // stamped — in one statement.
    sqlx::query!(
        r#"INSERT INTO chunk (id, title, user_id)
           SELECT 'filler-' || lpad(gs::text, 4, '0'), 'Filler ' || gs, $1
           FROM generate_series(1, 150) AS gs"#,
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // `chunk::list`'s default sort is `Newest`: `ORDER BY created_at DESC,
    // id ASC` — newest first. Giving this row an explicitly *older*
    // `created_at` than every filler (each of which took the column's own
    // `now()` default) makes it unambiguously the last of the 151 rows in
    // that order, regardless of the two statements' actual wall-clock
    // timing — well beyond position 100.
    sqlx::query!(
        r#"INSERT INTO chunk (id, title, user_id, created_at)
           VALUES ('zzzz-target-chunk', 'Beyond Position 100', $1, now() - interval '1 day')"#,
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let res = get(
        app.clone(),
        &cookie,
        "/api/chunks/export/context?maxTokens=1000000&format=json",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let titles: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();

    assert!(
        titles.contains(&"Beyond Position 100"),
        "a chunk ranked 151st must still be considered once the fetch width \
         is 500, not silently dropped by a 100-row cap: {titles:?}"
    );
}

/// `generate_claude_md`'s `chunk::list_internal` call (`claude_md.rs`,
/// `CLAUDE_MD_FETCH_LIMIT = 2000`) must see a tagged chunk ranked 151st by
/// title, which a 100-row cap would silently drop.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn claude_md_considers_more_than_100_tagged_chunks(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "claude-md-wide@b.test", "Claude Wide").await;
    let user_id = user_id_for_email(&pool, "claude-md-wide@b.test").await;

    let t = tag::find_or_create(&pool, &user_id, "claude-context")
        .await
        .unwrap();

    // 150 filler chunks, all titled "AAA Filler NNNN" so they sort first
    // under `Sort::Alpha`'s `ORDER BY title ASC`.
    sqlx::query!(
        r#"INSERT INTO chunk (id, title, user_id)
           SELECT 'filler-' || lpad(gs::text, 4, '0'), 'AAA Filler ' || lpad(gs::text, 4, '0'), $1
           FROM generate_series(1, 150) AS gs"#,
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Sorts after every filler title, so it's row 151 of 151 under
    // `ORDER BY title ASC` — well beyond position 100.
    sqlx::query!(
        r#"INSERT INTO chunk (id, title, user_id)
           VALUES ('zzzz-target-chunk', 'ZZZ Target Chunk', $1)"#,
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query!(
        r#"INSERT INTO chunk_tag (chunk_id, tag_id)
           SELECT id, $1 FROM chunk WHERE user_id = $2"#,
        t.id,
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let res = get(app.clone(), &cookie, "/api/chunks/export/claude-md").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let content = body["content"].as_str().unwrap();

    assert_eq!(
        body["chunks"].as_u64().unwrap(),
        151,
        "all 151 tagged chunks must be counted, not silently capped at 100"
    );
    assert!(
        content.contains("ZZZ Target Chunk"),
        "a tagged chunk ranked 151st by title must still be considered once \
         the fetch width is 2000, not silently dropped by a 100-row cap: {content}"
    );
}
