mod support;

use fubbik_cli::client::Space;
use sqlx::PgPool;
use support::TestApplication;

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cli_creates_space_tag_and_chunk_against_the_real_api(pool: PgPool) {
    // Given
    let app = TestApplication::start(pool).await;

    // When
    let space = app.run_cli(&["space", "add", "Integration"]).await;
    let tag = app.run_cli(&["tag", "add", "contract"]).await;
    let chunk = app
        .run_cli(&[
            "chunk",
            "add",
            "Rust path",
            "--content",
            "Served by the Rust API",
            "--space",
            space["id"].as_str().unwrap(),
        ])
        .await;

    // Then
    assert_eq!(tag["name"], "contract");
    assert_eq!(chunk["title"], "Rust path");
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk WHERE id = $1")
        .bind(chunk["id"].as_str().unwrap())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(stored, 1);
    assert_eq!(
        app.client
            .get_chunk(chunk["id"].as_str().unwrap())
            .await
            .unwrap()
            .title,
        "Rust path"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cli_matrix_lifecycle_persists_the_view(pool: PgPool) {
    // Given
    let app = TestApplication::start(pool).await;

    // When
    let matrix = app
        .run_cli(&["matrix", "create", "HTTP behavior", "--layer", "contract"])
        .await;
    let id = matrix["id"].as_str().unwrap();
    let dimension = app.run_cli(&["matrix", "add-dimension", id, "HTTP"]).await;
    let rule = app
        .run_cli(&["matrix", "add-rule", id, "Reject invalid input"])
        .await;
    let cell = app
        .run_cli(&[
            "matrix",
            "cell",
            id,
            rule["id"].as_str().unwrap(),
            dimension["id"].as_str().unwrap(),
        ])
        .await;
    let view = app.run_cli(&["matrix", "show", id]).await;

    // Then
    assert_eq!(cell["action"], "created");
    assert_eq!(view["summary"]["unspecified"], 1);
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM behavior_cell WHERE id = $1")
        .bind(cell["cell"]["id"].as_str().unwrap())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(stored, 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cli_links_requirement_to_plan(pool: PgPool) {
    // Given
    let app = TestApplication::start(pool).await;

    // When
    let requirement = app
        .run_cli(&[
            "req",
            "add",
            "Users can sign in",
            "--step",
            "given: an account",
            "--step",
            "when: valid credentials are submitted",
            "--step",
            "then: access is granted",
        ])
        .await;
    let plan = app.run_cli(&["plan", "create", "Authentication"]).await;
    app.run_cli(&[
        "plan",
        "link-requirement",
        plan["id"].as_str().unwrap(),
        requirement["id"].as_str().unwrap(),
    ])
    .await;
    let detail = app
        .run_cli(&["plan", "show", plan["id"].as_str().unwrap()])
        .await;

    // Then
    assert_eq!(detail["plan"]["title"], "Authentication");
    let links: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM plan_requirement WHERE plan_id = $1 AND requirement_id = $2",
    )
    .bind(plan["id"].as_str().unwrap())
    .bind(requirement["id"].as_str().unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(links, 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn setup_discovery_imports_documents_through_the_real_router(pool: PgPool) {
    // Given
    let app = TestApplication::start(pool).await;
    let space = app.run_cli(&["space", "add", "Documentation"]).await;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("README.md"),
        "# Operational guide\nUseful details.",
    )
    .unwrap();
    let discovery = fubbik_cli::setup::discover(root.path()).unwrap();
    let space = Space {
        id: space["id"].as_str().unwrap().into(),
        name: "Documentation".into(),
        kind: "codebase".into(),
        description: None,
    };

    // When
    let report = fubbik_cli::setup::import(&app.client, &space, &discovery).await;

    // Then
    assert_eq!(report.chunks_created, 1, "{:?}", report.errors);
    assert!(report.errors.is_empty());
    let stored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM chunk WHERE document_id IN \
         (SELECT id FROM document WHERE source_path = $1)",
    )
    .bind("README.md")
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(stored, 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cli_imports_a_document_and_exports_context(pool: PgPool) {
    // Given
    let app = TestApplication::start(pool).await;
    let space = app.run_cli(&["space", "add", "Operations"]).await;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("guide.md");
    std::fs::write(
        &source,
        "# Operations\nA reliable process for deployment and recovery.",
    )
    .unwrap();
    let source = source.to_string_lossy().into_owned();
    let space_id = space["id"].as_str().unwrap();

    // When
    app.run_cli(&["docs", "import", &source, "--space", space_id])
        .await;
    let context = app
        .run_cli(&[
            "context",
            "export",
            "--space",
            space_id,
            "--format",
            "json",
            "--max-tokens",
            "2000",
        ])
        .await;

    // Then
    let docs: i64 = sqlx::query_scalar("SELECT count(*) FROM document WHERE source_path = $1")
        .bind(&source)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(docs, 1);
    assert!(context.is_object());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cli_cleanup_preview_and_confirm_remove_a_near_empty_chunk(pool: PgPool) {
    // Given
    let app = TestApplication::start(pool).await;
    let chunk = app
        .run_cli(&["chunk", "add", "Throwaway", "--content", "short"])
        .await;
    let id = chunk["id"].as_str().unwrap();

    // When
    let preview = app.run_cli(&["cleanup"]).await;
    let applied = app.run_cli(&["cleanup", "--confirm"]).await;

    // Then
    assert_eq!(preview["summary"]["nearEmpty"], 1);
    assert_eq!(applied["removed"][0], id);
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk WHERE id = $1")
        .bind(id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cli_output_modes_share_chunk_task_and_status_results(pool: PgPool) {
    // Given a real API with one chunk and one quick task
    let app = TestApplication::start(pool).await;
    let chunk = app
        .run_cli(&[
            "chunk",
            "add",
            "Output contract",
            "--content",
            "Searchable text",
        ])
        .await;
    let chunk_id = chunk["id"].as_str().unwrap();
    let task = app.run_cli(&["task", "add", "Review output"]).await;
    let task_id = task["id"].as_str().unwrap();

    // When list, search, task, and status commands use each output mode
    let list_json = app.run_cli(&["list"]).await;
    let list_human = app.run_cli_with_flags(&[], &["list"]).await;
    let list_quiet = app.run_cli_with_flags(&["--quiet"], &["list"]).await;
    let search_quiet = app
        .run_cli_with_flags(&["--quiet"], &["search", "Output"])
        .await;
    let tasks_quiet = app
        .run_cli_with_flags(&["--quiet"], &["task", "list"])
        .await;
    let health_quiet = app.run_cli_with_flags(&["--quiet"], &["health"]).await;
    let stats_quiet = app.run_cli_with_flags(&["--quiet"], &["stats"]).await;
    let status_quiet = app.run_cli_with_flags(&["--quiet"], &["status"]).await;
    let stale_human = app.run_cli_with_flags(&[], &["stale", "list"]).await;

    // Then JSON, human, and quiet output describe the same stored resources
    for output in [
        &list_human,
        &list_quiet,
        &search_quiet,
        &tasks_quiet,
        &health_quiet,
        &stats_quiet,
        &status_quiet,
        &stale_human,
    ] {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(list_json[0]["id"], chunk_id);
    assert!(String::from_utf8_lossy(&list_human.stdout).contains("Output contract"));
    assert_eq!(
        String::from_utf8_lossy(&list_quiet.stdout),
        format!("{chunk_id}\n")
    );
    assert_eq!(
        String::from_utf8_lossy(&search_quiet.stdout),
        format!("{chunk_id}\n")
    );
    assert_eq!(
        String::from_utf8_lossy(&tasks_quiet.stdout),
        format!("{task_id}\n")
    );
    assert_eq!(String::from_utf8_lossy(&health_quiet.stdout), "ok\n");
    assert_eq!(String::from_utf8_lossy(&stats_quiet.stdout), "1\n");
    assert_eq!(String::from_utf8_lossy(&status_quiet.stdout), "ok\n");
    assert_eq!(
        String::from_utf8_lossy(&stale_human.stdout),
        "No stale chunks found.\n"
    );
}
