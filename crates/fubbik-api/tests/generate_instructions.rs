//! Tests for `GET /api/spaces/{id}/generate-instructions` — the port of
//! `packages/api/src/generate-instructions/{routes,service}.ts` (Task 10).
//!
//! Two deliberate divergences from Node this file's tests pin, both
//! explained in `crates/fubbik-api/src/generate_instructions/mod.rs`'s
//! module doc comment:
//!
//! - This port serves `/api/spaces/{id}/generate-instructions`, the path
//!   `apps/cli/src/commands/generate.ts:35,58,81` actually calls. Node
//!   only ever registers `/api/codebases/:id/generate-instructions`, so
//!   the CLI's three `generate` commands 404 against a real Node server.
//! - This port 404s when `id` isn't a space the caller owns
//!   (`generate_instructions_is_scoped_to_the_space_owner`). Node has no
//!   such check at all — an unowned/bogus `spaceId` silently falls back
//!   to rendering the caller's own global chunks.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use fubbik_db::repo::{chunk, space, tag};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn new_chunk(title: &str, content: &str) -> chunk::NewChunk {
    chunk::NewChunk {
        title: title.to_string(),
        content: content.to_string(),
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

async fn make_space(pool: &sqlx::PgPool, user_id: &str, name: &str) -> String {
    space::create(
        pool,
        user_id,
        space::NewSpace {
            name: name.to_string(),
            kind: "wiki".to_string(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id
}

/// Seeds a space with chunks that exercise every category the formatters
/// branch on: an "architecture"-tagged overview/architecture chunk, a
/// convention chunk with a rationale, and a commands chunk containing
/// `pnpm `. Enough spread that `claude`/`agents`/`cursor` — which include
/// different subsets of these categories (see each `format_*` function's
/// doc comment in `service.rs`) — are structurally forced to diverge.
async fn seed_categorized_chunks(pool: &sqlx::PgPool, user_id: &str, space_id: &str) {
    let arch = chunk::create(
        pool,
        user_id,
        chunk::NewChunk {
            chunk_type: "document".to_string(),
            ..new_chunk(
                "System Architecture",
                "How the pieces fit together in this system.",
            )
        },
    )
    .await
    .unwrap();
    let arch_tag = tag::find_or_create(pool, user_id, "architecture")
        .await
        .unwrap();
    tag::set_chunk_tags(pool, user_id, &arch.id, &[arch_tag.id])
        .await
        .unwrap();
    space::set_chunk_spaces(
        pool,
        user_id,
        &arch.id,
        std::slice::from_ref(&space_id.to_string()),
    )
    .await
    .unwrap();

    let mut convention = new_chunk("Naming Convention", "Use kebab-case for file names.");
    convention.rationale = Some("Consistency across the repo.".to_string());
    let convention = chunk::create(pool, user_id, convention).await.unwrap();
    space::set_chunk_spaces(
        pool,
        user_id,
        &convention.id,
        std::slice::from_ref(&space_id.to_string()),
    )
    .await
    .unwrap();

    let commands = chunk::create(
        pool,
        user_id,
        new_chunk("Build Commands", "Run `pnpm build` to build the project."),
    )
    .await
    .unwrap();
    space::set_chunk_spaces(
        pool,
        user_id,
        &commands.id,
        std::slice::from_ref(&space_id.to_string()),
    )
    .await
    .unwrap();
}

// ---------------------------------------------------------------------------
// each_format_produces_a_distinct_document
// ---------------------------------------------------------------------------

/// Asserts all three format bodies differ from one another, not merely
/// that each request returns 200. Asserting only 200 would pass even if
/// the format branch were wired wrong and every request returned the same
/// document — which would silently break two of the CLI's three
/// `generate` commands while every test stayed green. See Step 5's
/// mutation: routing all three formats through `format_claude` keeps every
/// `assert_eq!(res.status(), StatusCode::OK)` passing but must fail this
/// test.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn each_format_produces_a_distinct_document(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "formats@b.test", "Formats").await;
    let user_id = user_id_for_email(&pool, "formats@b.test").await;
    let space_id = make_space(&pool, &user_id, "formats-space").await;
    seed_categorized_chunks(&pool, &user_id, &space_id).await;

    // When
    let claude = get(
        app.clone(),
        &cookie,
        &format!("/api/spaces/{space_id}/generate-instructions?format=claude"),
    )
    .await;
    // Then
    assert_eq!(claude.status(), StatusCode::OK);
    let claude_content = json_body(claude).await["content"]
        .as_str()
        .unwrap()
        .to_string();

    let agents = get(
        app.clone(),
        &cookie,
        &format!("/api/spaces/{space_id}/generate-instructions?format=agents"),
    )
    .await;
    assert_eq!(agents.status(), StatusCode::OK);
    let agents_content = json_body(agents).await["content"]
        .as_str()
        .unwrap()
        .to_string();

    let cursor = get(
        app.clone(),
        &cookie,
        &format!("/api/spaces/{space_id}/generate-instructions?format=cursor"),
    )
    .await;
    assert_eq!(cursor.status(), StatusCode::OK);
    let cursor_content = json_body(cursor).await["content"]
        .as_str()
        .unwrap()
        .to_string();

    assert_ne!(
        claude_content, agents_content,
        "claude and agents documents must differ"
    );
    assert_ne!(
        claude_content, cursor_content,
        "claude and cursor documents must differ"
    );
    assert_ne!(
        agents_content, cursor_content,
        "agents and cursor documents must differ"
    );

    // Sanity-check the format-specific headers actually show up, so a
    // trivial "swap two words" mutation of one formatter can't slip a
    // wrong-but-still-distinct body past the three inequality checks
    // above.
    assert!(claude_content.starts_with("# CLAUDE.md"));
    assert!(agents_content.starts_with("# AGENTS.md"));
    assert!(cursor_content.starts_with("# .cursorrules"));
}

// ---------------------------------------------------------------------------
// unknown_format_is_rejected_or_defaults
// ---------------------------------------------------------------------------

/// Node's query schema is `t.Union([t.Literal("claude"),
/// t.Literal("agents"), t.Literal("cursor")])`
/// (`packages/api/src/generate-instructions/routes.ts:21`), which Elysia
/// rejects a request against *before* the handler body ever runs for any
/// value outside those three literals — Node does not fall back to a
/// default for a bad `format`, it rejects. This port matches "reject": an
/// unrecognised `format` fails to deserialize as `InstructionFormat`,
/// which `extract::Query` turns into `AppError::Validation` -> `400`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unknown_format_is_rejected_or_defaults(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "badformat@b.test", "Bad Format").await;
    let user_id = user_id_for_email(&pool, "badformat@b.test").await;
    let space_id = make_space(&pool, &user_id, "bad-format-space").await;

    // When
    let res = get(
        app.clone(),
        &cookie,
        &format!("/api/spaces/{space_id}/generate-instructions?format=bogus"),
    )
    .await;

    // Then
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "an unrecognised format must be rejected, not silently defaulted"
    );
}

/// Omitting `format` entirely must still succeed and default to `claude`
/// (`service.ts:30`'s `query.format ?? "claude"`) — this is the "absent"
/// case `unknown_format_is_rejected_or_defaults` doesn't cover.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn missing_format_defaults_to_claude(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "noformat@b.test", "No Format").await;
    let user_id = user_id_for_email(&pool, "noformat@b.test").await;
    let space_id = make_space(&pool, &user_id, "no-format-space").await;

    // When
    let res = get(
        app.clone(),
        &cookie,
        &format!("/api/spaces/{space_id}/generate-instructions"),
    )
    .await;

    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["format"], "claude");
}

// ---------------------------------------------------------------------------
// codebases alias parity
// ---------------------------------------------------------------------------

/// Node's only real registration is `/api/codebases/:id/generate-
/// instructions` (`packages/api/src/generate-instructions/routes.ts:8`);
/// this port keeps that path alive as a deprecated alias pointed at the
/// same handler as the primary `/api/spaces/...` route (see
/// `generate_instructions_codebases_alias_route`'s doc comment in
/// `routes.rs`). Asserts the alias returns the *same* body as the primary
/// route for the same space and format — without this, a later cleanup
/// could delete the alias (or let it drift, e.g. by wiring it to a
/// different handler) and nothing would catch it until an external
/// consumer complained.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn codebases_alias_matches_spaces_route(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alias@b.test", "Alias").await;
    let user_id = user_id_for_email(&pool, "alias@b.test").await;
    let space_id = make_space(&pool, &user_id, "alias-space").await;
    seed_categorized_chunks(&pool, &user_id, &space_id).await;

    // When
    let primary = get(
        app.clone(),
        &cookie,
        &format!("/api/spaces/{space_id}/generate-instructions?format=agents"),
    )
    .await;
    // Then
    assert_eq!(primary.status(), StatusCode::OK);
    let primary_body = json_body(primary).await;

    let alias = get(
        app.clone(),
        &cookie,
        &format!("/api/codebases/{space_id}/generate-instructions?format=agents"),
    )
    .await;
    assert_eq!(alias.status(), StatusCode::OK);
    let alias_body = json_body(alias).await;

    assert_eq!(
        primary_body, alias_body,
        "the /api/codebases alias must return the same body as /api/spaces for the same space and format"
    );
}

// ---------------------------------------------------------------------------
// generate_instructions_is_scoped_to_the_space_owner
// ---------------------------------------------------------------------------

/// User B requesting user A's space must be rejected, AND user A must
/// still be able to generate for their own space afterwards — the second
/// half is what proves the request was actually rejected rather than the
/// space (or its chunks) having been damaged by B's request.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn generate_instructions_is_scoped_to_the_space_owner(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));

    let cookie_a = signup(app.clone(), "owner-a@b.test", "Owner A").await;
    let user_a = user_id_for_email(&pool, "owner-a@b.test").await;
    let space_a = make_space(&pool, &user_a, "as-space").await;
    seed_categorized_chunks(&pool, &user_a, &space_a).await;

    let cookie_b = signup(app.clone(), "intruder-b@b.test", "Intruder B").await;

    // When
    // B requests A's space.
    let res = get(
        app.clone(),
        &cookie_b,
        &format!("/api/spaces/{space_a}/generate-instructions?format=claude"),
    )
    .await;
    // Then
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "user B must be rejected when requesting user A's space"
    );

    // A can still generate for their own space afterwards — proves B's
    // rejected request didn't damage the space or its chunks.
    let res = get(
        app.clone(),
        &cookie_a,
        &format!("/api/spaces/{space_a}/generate-instructions?format=claude"),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "user A must still be able to generate for their own space"
    );
    let body = json_body(res).await;
    let content = body["content"].as_str().unwrap();
    assert!(
        content.contains("System Architecture"),
        "A's own space content must still render intact: {content}"
    );
}

/// A bogus/nonexistent space id must also 404, not just a foreign-owner
/// one — pins that the check is "does this space resolve for me", not
/// merely "does this id belong to someone else".
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn nonexistent_space_is_rejected(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "nospace@b.test", "No Space").await;

    // When
    let res = get(
        app.clone(),
        &cookie,
        "/api/spaces/does-not-exist/generate-instructions?format=claude",
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
