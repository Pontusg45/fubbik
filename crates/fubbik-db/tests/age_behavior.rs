use fubbik_db::age::{self, BehaviorRuleVertex};

fn rule(id: &str, title: &str) -> BehaviorRuleVertex {
    BehaviorRuleVertex {
        id: id.into(),
        title: title.into(),
        layer: "invariant".into(),
        matrix_id: "m1".into(),
        category: "auth".into(),
    }
}

#[sqlx::test]
async fn upsert_behavior_rule_is_idempotent_and_updates_props(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    age::upsert_behavior_rule(&pool, &rule("r1", "First")).await.unwrap();
    age::upsert_behavior_rule(&pool, &rule("r1", "Renamed")).await.unwrap();

    let rules = age::list_behavior_rule_vertices(&pool).await.unwrap();
    // MERGE on id, not CREATE: running twice must not stack a second vertex.
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].title, "Renamed");
    assert_eq!(rules[0].matrix_id, "m1");
    assert_eq!(rules[0].category, "auth");
}

#[sqlx::test]
async fn behavior_rule_title_survives_quotes_and_apostrophes(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    age::upsert_behavior_rule(&pool, &rule("r1", r#"it's a "quoted" rule"#))
        .await
        .unwrap();

    let rules = age::list_behavior_rule_vertices(&pool).await.unwrap();
    assert_eq!(rules.len(), 1, "an apostrophe must not break the Cypher literal");
    assert_eq!(rules[0].title, r#"it's a "quoted" rule"#);
}

#[sqlx::test]
async fn governs_edges_rebuild_without_duplicating(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    age::cypher(&pool, "CREATE (:code_file {id: 'src/auth/session.ts'})")
        .await
        .unwrap();
    age::upsert_behavior_rule(&pool, &rule("r1", "Sessions expire")).await.unwrap();

    for _ in 0..2 {
        age::delete_governs_edges(&pool, "r1").await.unwrap();
        age::link_governs(&pool, "r1", "file", "auth/session.ts").await.unwrap();
    }

    let edges = age::list_governs_edges(&pool).await.unwrap();
    assert_eq!(edges.len(), 1, "delete-then-relink must not accumulate edges");
    assert_eq!(edges[0].source_id, "r1");
    assert_eq!(edges[0].target_id, "src/auth/session.ts");
    assert_eq!(edges[0].kind, "file");
}

#[sqlx::test]
async fn link_governs_is_a_noop_when_no_code_vertex_matches(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    age::upsert_behavior_rule(&pool, &rule("r1", "Orphan")).await.unwrap();
    // No code_file vertices exist at all — the normal state of this system,
    // since code-index is not ported (see the spec). MATCH finds nothing and
    // MERGE never runs; this must not error.
    age::link_governs(&pool, "r1", "file", "auth/session.ts").await.unwrap();

    assert!(age::list_governs_edges(&pool).await.unwrap().is_empty());
}
