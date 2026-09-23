//! HTTP-level tests for the `documents` domain.
//!
//! Cross-user SQL ownership guards (`find_by_id`/`update`/`delete`) are
//! proven load-bearing at the repo level in `fubbik-db/tests/document.rs`
//! — an API-level test can't distinguish "the SQL guard caught it" from "a
//! redundant caller-side check already 404'd" (this domain deliberately has
//! no such redundant check, see `documents::service`'s doc comments). What
//! THIS file proves instead: response shapes match Node's actual return
//! expressions (including the flattened `GET /{id}` shape and the
//! stale-`document`-snapshot quirk in sync results), status codes, and that
//! a cross-user request still 404s end-to-end while leaving victim data
//! unchanged.

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

async fn import(
    app: axum::Router,
    cookie: &str,
    source_path: &str,
    content: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/documents/import")
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({ "sourcePath": source_path, "content": content }).to_string(),
            ))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn sync(
    app: axum::Router,
    cookie: &str,
    id: &str,
    content: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::post(format!("/api/documents/{id}/sync"))
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({ "content": content }).to_string(),
            ))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_document(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/documents/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn render(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/documents/{id}/render"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/documents/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn list(app: axum::Router, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::get("/api/documents")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn search(app: axum::Router, cookie: &str, q: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/documents/search?q={q}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

const DOC_MD: &str = "# Guide\n\n## Setup\n\nInstall it.\n\n## Usage\n\nRun it.\n";

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn import_creates_document_and_sections(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-import@b.test", "Alice").await;

    // When
    let res = import(app.clone(), &cookie, "docs/guide.md", DOC_MD).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["status"], "created");
    assert_eq!(body["created"], 2);
    assert_eq!(body["updated"], 0);
    assert_eq!(body["document"]["title"], "Guide");
    assert!(body["firstChunkId"].is_string());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn reimporting_identical_content_is_unchanged(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-unchanged@b.test", "Alice").await;

    import(app.clone(), &cookie, "docs/guide.md", DOC_MD).await;
    // When
    let res = import(app.clone(), &cookie, "docs/guide.md", DOC_MD).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["status"], "unchanged");
    assert_eq!(body["created"], 0);
    assert_eq!(body["updated"], 0);
}

/// Proves the documented Node quirk: a `synced` result's `document` field
/// is the **pre-sync** snapshot (old title), even though the database row
/// was actually updated to the new title — see `documents::service::sync_document`'s
/// doc comment. A follow-up `GET /api/documents/{id}` shows the real,
/// updated title.
///
/// Uses a content change that adds a section without orphaning any
/// existing one (see `resyncing_after_a_section_is_removed_and_a_later_one_reuses_its_slot_500s`
/// below for the scenario that *does* orphan a section — that one hits an
/// unrelated, genuinely shared Node/Rust bug this test deliberately avoids
/// so it can isolate the stale-snapshot assertion).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn reimporting_changed_content_syncs_and_returns_stale_document_snapshot(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-sync@b.test", "Alice").await;

    let created = json_body(import(app.clone(), &cookie, "docs/guide.md", DOC_MD).await).await;
    // When
    let id = created["document"]["id"].as_str().unwrap().to_string();
    // Then
    assert_eq!(created["document"]["title"], "Guide");

    let changed_md = "# Renamed Guide\n\n## Setup\n\nInstall it differently.\n\n## Usage\n\nRun it.\n\n## Extra\n\nMore info.\n";
    let res = import(app.clone(), &cookie, "docs/guide.md", changed_md).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["status"], "synced");
    assert_eq!(body["created"], 1, "the new Extra section");
    assert_eq!(body["updated"], 1, "Setup's content changed");
    assert_eq!(
        body["document"]["title"], "Guide",
        "sync result must echo the pre-sync title, not the freshly-synced one"
    );

    let detail = json_body(get_document(app.clone(), &cookie, &id).await).await;
    assert_eq!(
        detail["title"], "Renamed Guide",
        "the actual stored document must reflect the new title"
    );
}

/// **Discovered while porting, not a regression introduced by this port**:
/// Node's `syncDocument` orphans a removed section's chunk by tagging it
/// `"stale"` but — due to the dead `documentOrder: undefined` update
/// documented on `fubbik_db::repo::document::touch_chunk` — never actually
/// clears its `document_order`. If a *later* re-sync then creates a brand
/// new section that happens to land on that same numeric slot, the
/// `INSERT` collides with the still-occupied `(document_id, document_order)`
/// unique index (`chunk_document_order_idx`, `0001_init.sql`) and the
/// request 500s. This is exactly reproducible against Node itself (same
/// schema, same buggy update, same unique index) — not specific to this
/// Rust port. Documented here as a known, pre-existing defect rather than
/// silently patched, per this slice's "don't touch chunk/document coupling
/// semantics without sign-off" guidance.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn resyncing_after_a_section_is_removed_and_a_later_one_reuses_its_slot_500s(
    pool: sqlx::PgPool,
) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-slot-collision@b.test", "Alice").await;

    // Setup=0, Usage=1.
    let created = json_body(import(app.clone(), &cookie, "docs/guide.md", DOC_MD).await).await;
    let id = created["document"]["id"].as_str().unwrap().to_string();

    // Usage's heading is gone -> its chunk goes stale, order 1 left set.
    let step2 = "# Guide\n\n## Setup\n\nStill here.\n";
    // When
    let res = sync(app.clone(), &cookie, &id, step2).await;
    // Then
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "removing a section alone does not collide"
    );

    // A brand new section lands back on order 1 -> collides with the
    // still-occupied slot from the orphaned "Usage" chunk.
    let step3 = "# Guide\n\n## Setup\n\nStill here.\n\n## Brand New\n\nTakes slot 1.\n";
    let res = sync(app.clone(), &cookie, &id, step3).await;
    assert_eq!(
        res.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "a real, Node-shared unique-index collision — see this test's doc comment"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn import_dir_imports_every_file(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-dir@b.test", "Alice").await;

    // When
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/documents/import-dir")
                .header("cookie", cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "files": [
                            {"sourcePath": "docs/a.md", "content": "# A\n\nContent A.\n"},
                            {"sourcePath": "docs/b.md", "content": "# B\n\nContent B.\n"}
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["document"]["title"], "A");
    assert_eq!(arr[1]["document"]["title"], "B");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn render_round_trips_markdown(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-render@b.test", "Alice").await;

    let created = json_body(import(app.clone(), &cookie, "docs/guide.md", DOC_MD).await).await;
    let id = created["document"]["id"].as_str().unwrap();

    // When
    let res = render(app.clone(), &cookie, id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let markdown = body["markdown"].as_str().unwrap();
    assert!(markdown.contains("## Setup"));
    assert!(markdown.contains("Install it."));
    assert!(markdown.contains("## Usage"));
    assert_eq!(body["document"]["title"], "Guide");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_document_returns_flattened_shape_with_chunks(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-detail@b.test", "Alice").await;

    let created = json_body(import(app.clone(), &cookie, "docs/guide.md", DOC_MD).await).await;
    let id = created["document"]["id"].as_str().unwrap();

    // When
    let res = get_document(app.clone(), &cookie, id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    // Flattened: document fields sit at the top level, not nested under
    // a `document` key.
    assert_eq!(body["title"], "Guide");
    assert_eq!(body["sourcePath"], "docs/guide.md");
    assert!(
        body.get("document").is_none(),
        "must be flattened, not nested"
    );
    assert_eq!(body["chunks"].as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_returns_bare_array_scoped_to_caller(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-list@b.test", "Bob").await;

    import(app.clone(), &alice_cookie, "docs/a.md", "# A\n\nContent.\n").await;
    import(app.clone(), &bob_cookie, "docs/b.md", "# B\n\nContent.\n").await;

    // When
    let res = list(app.clone(), &alice_cookie).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(body.is_array());
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"A"));
    assert!(
        !titles.contains(&"B"),
        "must not leak another user's document"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn search_rejects_short_queries_with_400(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-search@b.test", "Alice").await;

    // When
    let res = search(app.clone(), &cookie, "a").await;
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn search_finds_matching_chunks(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-search2@b.test", "Alice").await;

    import(
        app.clone(),
        &cookie,
        "docs/auth.md",
        "# Auth\n\n## Setup\n\nConfigure the auth provider.\n",
    )
    .await;

    // When
    let res = search(app.clone(), &cookie, "auth").await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(!body.as_array().unwrap().is_empty());
}

/// The deleted row is returned directly, not a `{message: "Deleted"}`
/// wrapper — matching Node's route (see `documents::routes::delete_document`'s
/// doc comment).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_returns_deleted_row_and_orphans_chunks(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-delete@b.test", "Alice").await;

    let created = json_body(import(app.clone(), &cookie, "docs/guide.md", DOC_MD).await).await;
    let id = created["document"]["id"].as_str().unwrap().to_string();

    // When
    let res = delete(app.clone(), &cookie, &id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["id"], id);
    assert_eq!(body["title"], "Guide");
    assert!(
        body.get("message").is_none(),
        "must not be a message wrapper"
    );

    let get_res = get_document(app.clone(), &cookie, &id).await;
    assert_eq!(get_res.status(), StatusCode::NOT_FOUND);

    let orphaned_count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM chunk WHERE document_id IS NULL AND user_id = (SELECT id FROM "user" WHERE email = 'alice-delete@b.test')"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        orphaned_count, 2,
        "sections must survive as orphaned chunks"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_get_sync_render_delete_all_404_and_leave_victim_unchanged(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-x@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-x@b.test", "Bob").await;

    let created =
        json_body(import(app.clone(), &alice_cookie, "docs/guide.md", DOC_MD).await).await;
    // When
    let id = created["document"]["id"].as_str().unwrap().to_string();

    // Then
    assert_eq!(
        get_document(app.clone(), &bob_cookie, &id).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        render(app.clone(), &bob_cookie, &id).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        sync(app.clone(), &bob_cookie, &id, "# Hijacked\n\nNope.\n")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        delete(app.clone(), &bob_cookie, &id).await.status(),
        StatusCode::NOT_FOUND
    );

    let still_alice = json_body(get_document(app.clone(), &alice_cookie, &id).await).await;
    assert_eq!(
        still_alice["title"], "Guide",
        "Alice's document must be untouched"
    );
}
