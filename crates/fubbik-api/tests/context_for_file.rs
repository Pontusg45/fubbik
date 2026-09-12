//! Tests for `fubbik_api::context_for_file` — the port of
//! `packages/api/src/context-for-file/service.ts`'s five-strategy chunk
//! matcher, plus the `context::resolvers::resolve_for_files` rewire that
//! now delegates to it.

use fubbik_api::context_for_file::dto::MatchReason;
use fubbik_api::context_for_file::service::get_context_for_file;
use fubbik_db::repo::{behavior_matrix as bm, chunk, chunk_meta, space, user};

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

/// An `OllamaClient` pointed at a closed port — every call fails fast
/// rather than hanging, standing in for "Ollama is not running".
fn unreachable_ai() -> fubbik_ai::OllamaClient {
    fubbik_ai::OllamaClient::new("http://127.0.0.1:1")
}

// ---------------------------------------------------------------------------
// file_ref_match_outranks_applies_to_match
// ---------------------------------------------------------------------------

/// **The most important test in this task.** Two chunks, otherwise
/// identical (same content, no connections, no rationale — so their
/// health-derived base scores are equal), one found only by the file-ref
/// strategy and one found only by applies-to. If the bonuses were merely
/// "present" rather than correctly ordered — e.g. both set to the same
/// value — this test would fail; asserting only that both chunks appear in
/// the result would not catch that.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn file_ref_match_outranks_applies_to_match(pool: sqlx::PgPool) {
    let user_id = seed_user(&pool, "bonus-order@b.test").await;

    let file_ref_chunk = chunk::create(&pool, &user_id, new_chunk("File-ref chunk"))
        .await
        .unwrap();
    chunk_meta::replace_file_refs(
        &pool,
        &file_ref_chunk.id,
        &user_id,
        &[chunk_meta::FileRefInput::from("src/nested/foo.rs")],
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

    let ai = unreachable_ai();
    let background = Default::default();
    let result = get_context_for_file(
        &pool,
        &ai,
        &background,
        &user_id,
        "src/nested/foo.rs",
        None,
        None,
    )
    .await
    .unwrap();

    let file_ref_result = result
        .chunks
        .iter()
        .find(|c| c.id == file_ref_chunk.id)
        .expect("file-ref chunk must be in the result");
    let applies_to_result = result
        .chunks
        .iter()
        .find(|c| c.id == applies_to_chunk.id)
        .expect("applies-to chunk must be in the result");

    assert_eq!(file_ref_result.match_reason, MatchReason::FileRef);
    assert_eq!(applies_to_result.match_reason, MatchReason::AppliesTo);

    // The two chunks are otherwise identical, so the ONLY thing that can
    // separate their scores is the strategy bonus — pinned to exactly the
    // documented +20/+10 difference, not just "greater than".
    assert_eq!(
        file_ref_result.score - applies_to_result.score,
        10.0,
        "file-ref (+20) must outscore applies-to (+10) by exactly the bonus difference: \
         file-ref={} applies-to={}",
        file_ref_result.score,
        applies_to_result.score
    );

    let file_ref_pos = result
        .chunks
        .iter()
        .position(|c| c.id == file_ref_chunk.id)
        .unwrap();
    let applies_to_pos = result
        .chunks
        .iter()
        .position(|c| c.id == applies_to_chunk.id)
        .unwrap();
    assert!(
        file_ref_pos < applies_to_pos,
        "the file-ref match must be sorted ahead of the applies-to match"
    );
}

// ---------------------------------------------------------------------------
// each_result_carries_its_match_reason
// ---------------------------------------------------------------------------

/// Pins the exact `matchReason` wire strings (`context-for-file/service.ts:27`'s
/// `"file-ref" | "applies-to" | "dependency" | "semantic" | "connected"`
/// union), serialized through `serde_json`, not just the Rust enum variant
/// name — a `MatchReason::FileRef` that accidentally serialized as
/// `"FileRef"` or `"fileRef"` would break every client of the `json-legacy`
/// format silently.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn each_result_carries_its_match_reason(pool: sqlx::PgPool) {
    let user_id = seed_user(&pool, "match-reason@b.test").await;

    let file_ref_chunk = chunk::create(&pool, &user_id, new_chunk("File-ref chunk"))
        .await
        .unwrap();
    chunk_meta::replace_file_refs(
        &pool,
        &file_ref_chunk.id,
        &user_id,
        &[chunk_meta::FileRefInput::from("src/nested/a.rs")],
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

    let ai = unreachable_ai();
    let background = Default::default();
    let result = get_context_for_file(
        &pool,
        &ai,
        &background,
        &user_id,
        "src/nested/a.rs",
        None,
        None,
    )
    .await
    .unwrap();

    let by_id = |id: &str| result.chunks.iter().find(|c| c.id == id).unwrap();

    let file_ref_json = serde_json::to_value(by_id(&file_ref_chunk.id)).unwrap();
    assert_eq!(file_ref_json["matchReason"], "file-ref");

    let applies_to_json = serde_json::to_value(by_id(&applies_to_chunk.id)).unwrap();
    assert_eq!(applies_to_json["matchReason"], "applies-to");
}

// ---------------------------------------------------------------------------
// semantic_strategy_is_skipped_when_ollama_is_unreachable
// ---------------------------------------------------------------------------

/// Node degrades rather than errors here: `generateQueryEmbedding(...).pipe(
/// Effect.flatMap(...), Effect.catchAll(() => Effect.succeed([])))`
/// (`service.ts:174-177`) swallows any failure — an unreachable Ollama
/// included — into an empty semantic result, and every other strategy
/// still runs to completion. This pins the same behaviour through the
/// HTTP route: 200, not 502, with the file-ref/applies-to results intact.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_strategy_is_skipped_when_ollama_is_unreachable(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "semantic-down@b.test", "Semantic Down").await;
    let user_id = user_id_for_email(&pool, "semantic-down@b.test").await;

    let file_ref_chunk = chunk::create(&pool, &user_id, new_chunk("Still Findable"))
        .await
        .unwrap();
    chunk_meta::replace_file_refs(
        &pool,
        &file_ref_chunk.id,
        &user_id,
        &[chunk_meta::FileRefInput::from("src/widget.rs")],
    )
    .await
    .unwrap();

    let res = get(
        app.clone(),
        &cookie,
        "/api/context/for-file?path=src/widget.rs&format=json-legacy",
    )
    .await;
    assert_eq!(
        res.status(),
        axum::http::StatusCode::OK,
        "an unreachable Ollama must degrade to 200, not surface as an error"
    );
    let body = json_body(res).await;
    let ids: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&file_ref_chunk.id.as_str()),
        "the file-ref strategy's result must survive a semantic-strategy failure: {ids:?}"
    );
}

// ---------------------------------------------------------------------------
// glob_matching_handles_star_and_double_star
// ---------------------------------------------------------------------------

/// Pins that this crate's `applies-to` matching goes through
/// `fubbik_core::glob::glob_match` — the Task 5 extraction that carries its
/// own 16 tests and the zero-depth `**` subtlety — rather than a
/// reimplementation local to this service. Mirrors the shapes
/// `glob-match.test.ts` exercises, at the seam this task actually touches.
#[test]
fn glob_matching_handles_star_and_double_star() {
    use fubbik_core::glob::glob_match;

    assert!(
        glob_match("src/*.rs", "src/main.rs"),
        "a single `*` must match one path segment"
    );
    assert!(
        !glob_match("src/*.rs", "src/nested/main.rs"),
        "a single `*` must NOT cross a `/` boundary"
    );
    assert!(
        glob_match("src/**/*.rs", "src/nested/deep/main.rs"),
        "`**` must match across any number of path segments"
    );
    // The literal `/`s surrounding `**` in `src/**/*.rs` are NOT consumed
    // by the globstar itself — Node's regex is `src/.*\/[^/]*\.rs`, which
    // needs a second, separate `/` after the globstar's expansion. A
    // zero-depth path has nowhere for that second `/` to come from, so it
    // does NOT match. Pinned here as well as in `fubbik-core::glob`'s own
    // `double_star_requires_a_real_directory_segment_either_side` — see
    // that test's doc comment for the fuller citation trail (an earlier,
    // never-actually-run version of this assertion asserted the opposite).
    assert!(
        !glob_match("src/**/*.rs", "src/main.rs"),
        "`**`'s surrounding literal `/`s require a real intermediate directory segment"
    );
}

// ---------------------------------------------------------------------------
// dependency_detection_matches_a_space_name
// ---------------------------------------------------------------------------

/// Ports `depMatchesCodebase` (`service.ts:63-70`) behaviour end to end:
/// a `deps` entry naming a space (exactly, or via a scoped package's last
/// path segment) pulls that space's most-recently-updated chunks in via
/// the dependency strategy. Exercised through the `json-legacy` HTTP route
/// with `deps=`, since that is the only place `deps` ever reaches
/// `get_context_for_file` (`resolve_for_files` never passes one — see its
/// doc comment).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn dependency_detection_matches_a_space_name(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "dep-match@b.test", "Dep Match").await;
    let user_id = user_id_for_email(&pool, "dep-match@b.test").await;

    let auth_space = space::create(
        &pool,
        &user_id,
        space::NewSpace {
            name: "auth".to_string(),
            kind: "code".to_string(),
            description: None,
        },
        None,
    )
    .await
    .unwrap();

    let dep_chunk = chunk::create(&pool, &user_id, new_chunk("Auth Space Chunk"))
        .await
        .unwrap();
    space::set_chunk_spaces(
        &pool,
        &user_id,
        &dep_chunk.id,
        std::slice::from_ref(&auth_space.id),
    )
    .await
    .unwrap();

    // `@acme/auth` matches the `auth` space via the scoped-package
    // last-segment rule, not an exact-string match.
    let res = get(
        app.clone(),
        &cookie,
        "/api/context/for-file?path=irrelevant/path.rs&format=json-legacy&deps=%40acme%2Fauth",
    )
    .await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);
    let body = json_body(res).await;
    let matches: Vec<(&str, &str)> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["id"].as_str().unwrap(),
                c["matchReason"].as_str().unwrap(),
            )
        })
        .collect();

    assert!(
        matches
            .iter()
            .any(|(id, reason)| *id == dep_chunk.id && *reason == "dependency"),
        "a dep naming the space (via scoped-package last segment) must pull in that \
         space's chunks with matchReason \"dependency\": {matches:?}"
    );
}

// ---------------------------------------------------------------------------
// resolve_for_files rewire — the binding requirement carried from Task 5.
// ---------------------------------------------------------------------------

/// Proves `resolve_for_files` (and therefore `/api/context/for-files`) now
/// reaches the semantic strategy, not just file-ref/applies-to. This is the
/// one test that would catch the rewire being skipped or silently
/// reverted: before Task 7, `resolve_for_files` implemented only the two
/// highest-priority strategies directly, so a chunk findable ONLY by
/// semantic similarity (no file-ref, no applies-to pattern, no shared
/// space/dependency) would never have appeared here.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn for_files_finds_a_chunk_via_the_semantic_strategy(pool: sqlx::PgPool) {
    let mut query_vector = vec![0.0f32; 768];
    query_vector[0] = 1.0;
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/embeddings"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "embedding": query_vector })),
        )
        .mount(&server)
        .await;

    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(
        app.clone(),
        "semantic-delegation@b.test",
        "Semantic Delegation",
    )
    .await;
    let user_id = user_id_for_email(&pool, "semantic-delegation@b.test").await;

    // A chunk with an embedding identical to the mocked query vector, but
    // no file-ref, no applies-to pattern, and no space membership — the
    // ONLY strategy that can find it is semantic.
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id, embedding) \
         VALUES ($1, $2, $3, 'note', $4, $5::text::vector)",
    )
    .bind(fubbik_db::new_id())
    .bind("Semantic Only Chunk")
    .bind("x".repeat(200))
    .bind(&user_id)
    .bind({
        let mut parts = vec!["0"; 768];
        parts[0] = "1";
        format!("[{}]", parts.join(","))
    })
    .execute(&pool)
    .await
    .unwrap();

    let res = get(
        app.clone(),
        &cookie,
        "/api/context/for-files?paths=some/unrelated/path.rs&maxTokens=50000",
    )
    .await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);
    let body = json_body(res).await;
    let content = body["content"].as_str().unwrap();

    assert!(
        content.contains("Semantic Only Chunk"),
        "resolve_for_files must delegate to get_context_for_file and reach the semantic \
         strategy, not just file-ref/applies-to: {content}"
    );
}

// ---------------------------------------------------------------------------
// Review follow-up: the applies-to strategy's `chunk::list` call must see
// beyond the 100-row HTTP clamp (Finding 2, final whole-branch review).
// Bulk-inserts filler rows via a single SQL statement rather than 150+
// `chunk::create` round trips, matching the pattern used for the same
// review-follow-up tests in `tests/context_export.rs`.
// ---------------------------------------------------------------------------

/// `get_context_for_file`'s applies-to strategy (`service.rs`, step 2) must
/// consider more than the 100 newest chunks. `chunk::list` clamps to
/// `[1,100]` regardless of `params.limit`; the applies-to strategy must go
/// through `chunk::list_internal` instead (`limit: 1000`, matching Node's
/// uncapped `listChunks` call at `context-for-file/service.ts:97-101`) or a
/// chunk ranked 151st by `created_at` is silently unreachable via
/// applies-to, even with a matching glob pattern.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn applies_to_strategy_considers_more_than_100_qualifying_chunks(pool: sqlx::PgPool) {
    let user_id = seed_user(&pool, "applies-to-wide@b.test").await;

    // 150 filler chunks, ids "filler-0001".."filler-0150", all inserted (and
    // therefore `created_at`-stamped) in one statement.
    sqlx::query!(
        r#"INSERT INTO chunk (id, title, content, type, user_id, review_status)
           SELECT 'filler-' || lpad(gs::text, 4, '0'), 'Filler ' || gs, repeat('x', 200), 'note', $1, 'approved'
           FROM generate_series(1, 150) AS gs"#,
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // `chunk::list`'s default sort is `Newest`: `ORDER BY created_at DESC,
    // id ASC` — newest first. An explicitly *older* `created_at` than every
    // filler (each of which took the column's own `now()` default) makes
    // this chunk unambiguously the last of the 151 rows in that order —
    // well beyond position 100.
    let target = chunk::create(&pool, &user_id, new_chunk("Beyond Position 100"))
        .await
        .unwrap();
    sqlx::query!(
        "UPDATE chunk SET created_at = now() - interval '1 day' WHERE id = $1",
        target.id
    )
    .execute(&pool)
    .await
    .unwrap();
    chunk_meta::replace_applies_to(
        &pool,
        &target.id,
        &user_id,
        &[chunk_meta::AppliesToInput::from("wide/**/*.rs")],
    )
    .await
    .unwrap();

    let ai = unreachable_ai();
    let background = Default::default();
    let result = get_context_for_file(
        &pool,
        &ai,
        &background,
        &user_id,
        "wide/nested/foo.rs",
        None,
        None,
    )
    .await
    .unwrap();

    let found = result.chunks.iter().find(|c| c.id == target.id);
    assert!(
        found.is_some(),
        "a chunk ranked 151st must still be reachable via the applies-to strategy \
         once the fetch width is 1000, not silently dropped by a 100-row cap"
    );
    assert_eq!(found.unwrap().match_reason, MatchReason::AppliesTo);
}

// ---------------------------------------------------------------------------
// governing behaviors — Fix round 1
// ---------------------------------------------------------------------------

/// Seeds one behaviour rule linked, via a cell's `behavior_cell_code` row,
/// to the exact path being requested — the reverse lookup
/// `matrices::service::behaviors_for_path` performs. Asserts:
///
/// 1. A request for that path gets a `structured-md` response whose
///    `content` contains the `## Behaviors governing this file` heading
///    and the rule's title.
/// 2. A request for an UNRELATED path — no linked behaviour at all — gets
///    NEITHER that heading NOR a dangling blank-line separator artefact.
///    This half is the one that would catch a broken emptiness check:
///    appending unconditionally would leave a dangling separator (or an
///    empty heading section) even when there is nothing to append.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn for_file_appends_governing_behaviors_to_markdown(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "behaviors-md@b.test", "Behaviors Md").await;
    let user_id = user_id_for_email(&pool, "behaviors-md@b.test").await;

    let matrix = bm::create(
        &pool,
        &user_id,
        bm::NewMatrix {
            name: "Invariants".to_string(),
            layer: "invariant".to_string(),
            description: None,
            space_id: None,
        },
    )
    .await
    .unwrap()
    .expect("a matrix with no space_id always inserts");
    let rule = bm::create_rule(
        &pool,
        &matrix.id,
        &user_id,
        bm::NewRule {
            title: "Never log secrets".to_string(),
            description: None,
            category: None,
            rationale: None,
            alternatives: None,
            consequences: None,
            counterexample: None,
        },
    )
    .await
    .unwrap()
    .unwrap();
    let dimension = bm::create_dimension(&pool, &matrix.id, &user_id, "Logging")
        .await
        .unwrap()
        .unwrap();
    let cell = bm::create_cell(&pool, &rule.id, &dimension.id, &matrix.id, &user_id)
        .await
        .unwrap()
        .unwrap();
    bm::link_cell_code(
        &pool,
        &cell.id,
        "file",
        "src/governed.rs",
        &matrix.id,
        &user_id,
    )
    .await
    .unwrap()
    .unwrap();

    // Half one: the governed path gets the section.
    let governed = get(
        app.clone(),
        &cookie,
        "/api/context/for-file?path=src/governed.rs",
    )
    .await;
    assert_eq!(governed.status(), StatusCode::OK);
    let governed_body = json_body(governed).await;
    let governed_content = governed_body["content"].as_str().unwrap();
    assert!(
        governed_content.contains("## Behaviors governing this file"),
        "a path with a linked behaviour must get the governing-behaviors heading: {governed_content}"
    );
    assert!(
        governed_content.contains("Never log secrets"),
        "the linked rule's title must appear: {governed_content}"
    );

    // Half two: an unrelated path gets neither the heading nor a dangling
    // separator artefact.
    let unrelated = get(
        app.clone(),
        &cookie,
        "/api/context/for-file?path=src/unrelated.rs",
    )
    .await;
    assert_eq!(unrelated.status(), StatusCode::OK);
    let unrelated_body = json_body(unrelated).await;
    let unrelated_content = unrelated_body["content"].as_str().unwrap();
    assert!(
        !unrelated_content.contains("## Behaviors governing this file"),
        "a path with no linked behaviour must NOT get the heading: {unrelated_content}"
    );
    assert!(
        !unrelated_content.ends_with("\n\n"),
        "an empty behaviors section must not leave a dangling blank-line separator: {unrelated_content:?}"
    );
}

// ---------------------------------------------------------------------------
// HTTP test harness — mirrors `crates/fubbik-api/tests/context.rs`.
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
