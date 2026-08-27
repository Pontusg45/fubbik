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
use fubbik_db::repo::{chunk, chunk_meta, plan, user};

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
    let ids = fubbik_api::context::resolvers::resolve_for_files(&pool, &user_id, &paths, None)
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
