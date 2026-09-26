use fubbik_db::seed::{SeedOptions, SeedScenario, seed, seed_core};
use sqlx::{PgPool, Row};

#[sqlx::test]
async fn core_catalog_is_idempotent_and_links_inverse_relations(pool: PgPool) {
    // Given a fresh migrated database
    let mut tx = pool.begin().await.unwrap();

    // When the builtin catalog is seeded twice
    let first = seed_core(&mut tx).await.unwrap();
    let second = seed_core(&mut tx).await.unwrap();
    tx.commit().await.unwrap();

    // Then counts and inverse references remain stable
    assert_eq!(first, second);
    assert_eq!(first.chunk_types, 7);
    assert_eq!(first.connection_relations, 13);
    assert_eq!(first.templates, 5);
    for (table, expected) in [
        ("chunk_type", 7_i64),
        ("connection_relation", 13),
        ("chunk_template", 5),
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, expected, "{table}");
    }
    let inverse_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM connection_relation r \
         JOIN connection_relation i ON i.id = r.inverse_of_id AND i.inverse_of_id = r.id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(inverse_count, 10);
    let row = sqlx::query("SELECT inverse_of_id FROM connection_relation WHERE id = 'part_of'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("inverse_of_id"), "contains");
}

#[sqlx::test]
async fn core_catalog_rolls_back_with_parent_transaction(pool: PgPool) {
    // Given a fresh migrated database and an uncommitted seed transaction
    let mut tx = pool.begin().await.unwrap();
    seed_core(&mut tx).await.unwrap();
    sqlx::query("INSERT INTO chunk_type (id, label) VALUES ('seed-rollback-probe', 'probe')")
        .execute(&mut *tx)
        .await
        .unwrap();

    // When the caller rolls the transaction back
    tx.rollback().await.unwrap();

    // Then changes made by that transaction do not persist
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM chunk_type WHERE id = 'seed-rollback-probe'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test]
async fn selected_core_module_runs_through_transactional_entry_point(pool: PgPool) {
    // Given an explicit request for the ported core module
    let options = SeedOptions {
        scenario: SeedScenario::Minimal,
        only: Some(["core".to_string()].into()),
        ..SeedOptions::default()
    };

    // When the Rust seed command runs twice
    let first = seed(&pool, &options).await.unwrap();
    let second = seed(&pool, &options).await.unwrap();

    // Then the report and catalog remain stable
    assert_eq!(first.modules, ["core"]);
    assert_eq!(first.core, second.core);
    assert_eq!(first.core.unwrap().chunk_types, 7);
}

#[sqlx::test]
async fn unported_module_is_rejected_before_seeding(pool: PgPool) {
    // Given a default demo seed containing modules not yet ported
    let options = SeedOptions::default();

    // When the Rust seed entry point validates its plan
    let error = seed(&pool, &options).await.unwrap_err();

    // Then it reports the missing module without inserting catalog rows
    assert!(error.to_string().contains("has not been ported yet"));
}

#[sqlx::test]
async fn codebase_module_reuses_named_spaces_on_repeat_runs(pool: PgPool) {
    // Given a request for the codebase module and its core prerequisite
    let options = SeedOptions {
        only: Some(["codebases".to_string()].into()),
        ..SeedOptions::default()
    };

    // When the seed runs twice
    let first = seed(&pool, &options).await.unwrap();
    let second = seed(&pool, &options).await.unwrap();

    // Then it reports two spaces and does not duplicate either name
    assert_eq!(first.modules, ["core", "codebases"]);
    assert_eq!(second.spaces, Some(2));
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM space WHERE user_id = 'dev-user' \
         AND name IN ('fubbik', 'fubbik-docs')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2);
}

#[sqlx::test]
async fn tag_module_seeds_legacy_named_catalog_idempotently(pool: PgPool) {
    // Given a request for tags and their core prerequisite
    let options = SeedOptions {
        only: Some(["tags".to_string()].into()),
        ..SeedOptions::default()
    };

    // When the native seed runs twice
    let first = seed(&pool, &options).await.unwrap();
    let second = seed(&pool, &options).await.unwrap();

    // Then the exact legacy tag catalog and type associations are present
    assert_eq!(first.tags, second.tags);
    let counts = first.tags.unwrap();
    assert_eq!(counts.tag_types, 8);
    assert_eq!(counts.tags, 64);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tag t JOIN tag_type tt ON tt.id = t.tag_type_id \
         WHERE t.id LIKE 'seed-tag-%' AND tt.id LIKE 'seed-tt-%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 64);
}

#[sqlx::test]
async fn minimal_scenario_seeds_self_documentation_and_named_links(pool: PgPool) {
    // Given the minimal scenario with its dependency closure
    let options = SeedOptions {
        scenario: SeedScenario::Minimal,
        ..SeedOptions::default()
    };

    // When the complete minimal seed runs twice
    let first = seed(&pool, &options).await.unwrap();
    let second = seed(&pool, &options).await.unwrap();

    // Then the six descriptions and seven links remain stable
    assert_eq!(
        first.modules,
        ["core", "codebases", "tags", "self-documenting"]
    );
    assert_eq!(first.self_documenting, second.self_documenting);
    let counts = first.self_documenting.unwrap();
    assert_eq!(counts.chunks, 6);
    assert_eq!(counts.connections, 7);
    assert_eq!(counts.applies_to, 6);
    let links: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM chunk_connection WHERE id LIKE 'seed-selfdoc-link-%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(links, 7);
}

#[sqlx::test]
async fn chunk_module_ports_all_named_legacy_fixtures(pool: PgPool) {
    // Given the chunks module with its catalog, space, and tag prerequisites
    let options = SeedOptions {
        only: Some(["chunks".to_string()].into()),
        ..SeedOptions::default()
    };

    // When the Rust seed runs twice
    let first = seed(&pool, &options).await.unwrap();
    let second = seed(&pool, &options).await.unwrap();

    // Then all 46 legacy chunks and their named relationships remain stable
    assert_eq!(first.chunks, Some(46));
    assert_eq!(second.chunks, Some(46));
    let linked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM chunk_space cs JOIN chunk c ON c.id = cs.chunk_id \
         WHERE c.id LIKE 'seed-chunk-%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked, 46);
}

#[sqlx::test]
async fn connection_module_resolves_all_named_chunk_links(pool: PgPool) {
    // Given all chunk fixtures and a request for their graph topology
    let options = SeedOptions {
        only: Some(["connections".to_string()].into()),
        ..SeedOptions::default()
    };

    // When the native seed runs twice
    let first = seed(&pool, &options).await.unwrap();
    let second = seed(&pool, &options).await.unwrap();

    // Then 56 typed relationships connect existing chunk rows exactly once
    assert_eq!(first.connections, Some(56));
    assert_eq!(second.connections, Some(56));
    let linked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM chunk_connection cc \
         JOIN chunk source ON source.id = cc.source_id \
         JOIN chunk target ON target.id = cc.target_id \
         WHERE cc.id LIKE 'seed-connection-%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked, 56);
}

#[sqlx::test]
async fn file_links_module_ports_globs_and_direct_refs(pool: PgPool) {
    // Given the named chunks required by file-level discovery
    let options = SeedOptions {
        only: Some(["file-links".to_string()].into()),
        ..SeedOptions::default()
    };

    // When file links seed twice
    let first = seed(&pool, &options).await.unwrap();
    let second = seed(&pool, &options).await.unwrap();

    // Then all legacy globs and paths are present exactly once
    assert_eq!(first.file_links, second.file_links);
    let counts = first.file_links.unwrap();
    assert_eq!(counts.applies_to, 52);
    assert_eq!(counts.file_refs, 10);
    let applies: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM chunk_applies_to WHERE id LIKE 'seed-file-applies-%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(applies, 52);
}

#[sqlx::test]
async fn requirements_link_named_use_cases_and_chunks(pool: PgPool) {
    // Given the two use cases and named knowledge chunks
    let options = SeedOptions {
        only: Some(["requirements".to_string()].into()),
        ..SeedOptions::default()
    };

    // When requirements seed twice
    let first = seed(&pool, &options).await.unwrap();
    let second = seed(&pool, &options).await.unwrap();

    // Then two BDD requirements and their three chunk references remain stable
    assert_eq!(first.use_cases, Some(2));
    assert_eq!(first.requirements, Some(2));
    assert_eq!(second.requirements, Some(2));
    let linked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM requirement_chunk rc \
         JOIN requirement r ON r.id = rc.requirement_id \
         WHERE r.id LIKE 'seed-requirement-%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked, 3);
}
