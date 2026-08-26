use fubbik_db::repo::{chunk, graph, space, tag, tag_type, user, workspace};

async fn a_chunk(pool: &sqlx::PgPool, uid: &str, title: &str) -> String {
    chunk::create(
        pool,
        uid,
        chunk::NewChunk {
            title: title.into(),
            content: String::new(),
            chunk_type: "note".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id
}

async fn link_space(pool: &sqlx::PgPool, chunk_id: &str, space_id: &str) {
    sqlx::query!(
        "INSERT INTO chunk_space (chunk_id, space_id) VALUES ($1, $2)",
        chunk_id,
        space_id
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn a_space(pool: &sqlx::PgPool, uid: &str, name: &str) -> String {
    space::create(
        pool,
        uid,
        space::NewSpace {
            name: name.into(),
            kind: "code".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id
}

#[sqlx::test]
async fn chunk_meta_unscoped_returns_only_this_users_chunks(pool: sqlx::PgPool) {
    let mine = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let theirs = user::create(&pool, "c@d.test", "C", None).await.unwrap().id;
    a_chunk(&pool, &mine, "Mine").await;
    a_chunk(&pool, &theirs, "Theirs").await;

    let rows = graph::list_chunk_meta(&pool, &mine, None, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Mine");
}

#[sqlx::test]
async fn chunk_meta_space_scope_includes_global_chunks(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let target = a_space(&pool, &uid, "Target").await;
    let other = a_space(&pool, &uid, "Other").await;

    let in_target = a_chunk(&pool, &uid, "In target").await;
    let in_other = a_chunk(&pool, &uid, "In other").await;
    a_chunk(&pool, &uid, "Global").await;
    link_space(&pool, &in_target, &target).await;
    link_space(&pool, &in_other, &other).await;

    let rows = graph::list_chunk_meta(&pool, &uid, Some(&target), None)
        .await
        .unwrap();
    let titles: Vec<&str> = rows.iter().map(|r| r.title.as_str()).collect();

    // The rule that is easiest to lose in translation from Drizzle's
    // `OR id NOT IN (SELECT chunk_id FROM chunk_space)`: a chunk belonging to
    // NO space is global and appears under every scope.
    assert!(titles.contains(&"In target"));
    assert!(
        titles.contains(&"Global"),
        "global chunks must survive space scoping"
    );
    assert!(!titles.contains(&"In other"));
}

#[sqlx::test]
async fn chunk_meta_workspace_scope_spans_member_spaces_and_wins_over_space_id(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let a = a_space(&pool, &uid, "A").await;
    let b = a_space(&pool, &uid, "B").await;
    let outside = a_space(&pool, &uid, "Outside").await;

    let ws = workspace::create(
        &pool,
        &uid,
        workspace::NewWorkspace {
            name: "WS".into(),
            description: None,
        },
    )
    .await
    .unwrap()
    .id;
    workspace::add_space(&pool, &uid, &ws, &a).await.unwrap();
    workspace::add_space(&pool, &uid, &ws, &b).await.unwrap();

    let in_a = a_chunk(&pool, &uid, "In A").await;
    let in_b = a_chunk(&pool, &uid, "In B").await;
    let in_outside = a_chunk(&pool, &uid, "In outside").await;
    link_space(&pool, &in_a, &a).await;
    link_space(&pool, &in_b, &b).await;
    link_space(&pool, &in_outside, &outside).await;

    // `outside` passed as space_id AND a workspace passed: Node's else-if
    // (packages/db/src/repository/graph.ts:13-25) means workspace wins and
    // space_id is ignored entirely.
    let rows = graph::list_chunk_meta(&pool, &uid, Some(&outside), Some(&ws))
        .await
        .unwrap();
    let titles: Vec<&str> = rows.iter().map(|r| r.title.as_str()).collect();

    assert!(titles.contains(&"In A"));
    assert!(titles.contains(&"In B"));
    assert!(
        !titles.contains(&"In outside"),
        "workspace must win over space_id"
    );
}

#[sqlx::test]
async fn connections_include_edges_pointing_at_my_chunks(pool: sqlx::PgPool) {
    let mine = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let theirs = user::create(&pool, "c@d.test", "C", None).await.unwrap().id;
    let m = a_chunk(&pool, &mine, "Mine").await;
    let t = a_chunk(&pool, &theirs, "Theirs").await;

    // Inbound from a chunk I do not own: Node matches on source OR target,
    // so this counts as mine.
    sqlx::query!(
        "INSERT INTO chunk_connection (id, source_id, target_id, relation)
         VALUES ('c1', $1, $2, 'related_to')",
        t,
        m
    )
    .execute(&pool)
    .await
    .unwrap();

    let rows = graph::list_connections(&pool, &mine).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].relation, "related_to");
}

#[sqlx::test]
async fn chunk_tags_keep_untyped_tags(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let c = a_chunk(&pool, &uid, "T").await;

    let tt = tag_type::create(&pool, &uid, "Layer", Some("#fff"), None)
        .await
        .unwrap()
        .id;
    let typed = tag::create(&pool, &uid, "backend", Some(&tt))
        .await
        .unwrap()
        .id;
    let untyped = tag::create(&pool, &uid, "loose", None).await.unwrap().id;

    for tag_id in [&typed, &untyped] {
        sqlx::query!(
            "INSERT INTO chunk_tag (chunk_id, tag_id) VALUES ($1, $2)",
            c,
            tag_id
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    let rows = graph::list_chunk_tags_with_types(&pool, &uid)
        .await
        .unwrap();
    // A LEFT join, not an inner one: an untyped tag must still appear.
    assert_eq!(rows.len(), 2);
    let loose = rows.iter().find(|r| r.tag_name == "loose").unwrap();
    assert!(loose.tag_type_id.is_none());
    assert!(loose.tag_type_name.is_none());
}

#[sqlx::test]
async fn chunk_space_mappings_are_scoped_by_space_owner(pool: sqlx::PgPool) {
    let mine = user::create(&pool, "a@b.test", "A", None).await.unwrap().id;
    let theirs = user::create(&pool, "c@d.test", "C", None).await.unwrap().id;

    let my_space = a_space(&pool, &mine, "Mine").await;
    let their_space = a_space(&pool, &theirs, "Theirs").await;
    let mc = a_chunk(&pool, &mine, "MC").await;
    let tc = a_chunk(&pool, &theirs, "TC").await;
    link_space(&pool, &mc, &my_space).await;
    link_space(&pool, &tc, &their_space).await;

    let rows = graph::list_chunk_space_mappings(&pool, &mine)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].space_name, "Mine");
}
