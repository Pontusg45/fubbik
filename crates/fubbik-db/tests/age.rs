use fubbik_db::age;
use sqlx::postgres::PgPoolOptions;

#[test]
fn esc_cypher_escapes_backslashes_before_quotes() {
    assert_eq!(age::esc_cypher(r"a\b"), r"a\\b");
    assert_eq!(age::esc_cypher("it's"), r"it\'s");
    // Order matters: escaping quotes first would double-escape the backslash.
    assert_eq!(age::esc_cypher(r"\'"), r"\\\'");
}

#[sqlx::test]
async fn cypher_round_trips_a_real_vertex(pool: sqlx::PgPool) {
    // Migration 0001 installs AGE and creates the 'knowledge' graph, so this
    // exercises real agtype output rather than the degradation path.
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping round-trip");
        return;
    }

    let created = age::cypher(
        &pool,
        "CREATE (n:chunk {title: 'from rust', code: 'a::b'}) RETURN n",
    )
    .await
    .unwrap();

    assert_eq!(created.len(), 1);
    let v = &created[0];
    assert_eq!(v["label"], "chunk");
    assert_eq!(v["properties"]["title"], "from rust");
    assert_eq!(
        v["properties"]["code"], "a::b",
        "property values containing :: must survive suffix stripping"
    );

    let matched = age::cypher(&pool, "MATCH (n:chunk) RETURN n")
        .await
        .unwrap();
    assert_eq!(matched.len(), 1);
}

#[sqlx::test]
async fn cypher_returns_scalars(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        return;
    }
    let rows = age::cypher(&pool, "RETURN 42").await.unwrap();
    assert_eq!(rows[0], 42);
}

#[sqlx::test]
async fn cypher_round_trips_a_real_edge(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping edge round-trip");
        return;
    }

    age::cypher(
        &pool,
        "CREATE (a:probe_rev {title: 'a'})-[r:REL_REV {kind: 'a::b'}]->(b:probe_rev {title: 'b'}) RETURN r",
    )
    .await
    .unwrap();

    let rows = age::cypher(&pool, "MATCH ()-[r:REL_REV]->() RETURN r")
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    let edge = &rows[0];
    assert_eq!(edge["label"], "REL_REV");
    assert_eq!(
        edge["properties"]["kind"], "a::b",
        "edge property values containing :: must survive suffix stripping"
    );
    assert!(edge["start_id"].is_i64() || edge["start_id"].is_u64());
    assert!(edge["end_id"].is_i64() || edge["end_id"].is_u64());
}

#[sqlx::test]
async fn cypher_round_trips_a_real_path(pool: sqlx::PgPool) {
    // This is the case the CRITICAL fix targets: AGE emits a path as a JSON
    // array where every nested vertex/edge carries its OWN `::vertex`/`::edge`
    // suffix in addition to the outer `::path` suffix. Stripping only the
    // trailing suffix (the original bug) leaves invalid JSON and the whole
    // row silently vanishes via `filter_map`.
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping path round-trip");
        return;
    }

    age::cypher(
        &pool,
        "CREATE (a:probe_rev {title: 'a'})-[r:REL_REV]->(b:probe_rev {title: 'b'})",
    )
    .await
    .unwrap();

    let rows = age::cypher(
        &pool,
        "MATCH p = (a:probe_rev)-[r:REL_REV]->(b:probe_rev) RETURN p",
    )
    .await
    .unwrap();

    assert_eq!(
        rows.len(),
        1,
        "path row must not be silently dropped by the parser"
    );
    let path = rows[0].as_array().expect("path must parse as a JSON array");
    assert_eq!(path.len(), 3, "path should contain vertex, edge, vertex");
    assert_eq!(path[0]["label"], "probe_rev");
    assert_eq!(path[1]["label"], "REL_REV");
    assert_eq!(path[2]["label"], "probe_rev");
}

/// `cypher()` used to `SET search_path` (session-scoped) on a pooled
/// connection and return it to the pool without resetting — the mutation
/// then persisted for whoever acquired that connection next. Pins a pool
/// to `max_connections(1)` so every acquisition in this test is guaranteed
/// to be the exact same physical connection, then proves `SHOW
/// search_path` is back to the un-mutated default immediately after a
/// `cypher()` call returns.
#[sqlx::test]
async fn cypher_does_not_leak_search_path_onto_the_pooled_connection(pool: sqlx::PgPool) {
    let opts = (*pool.connect_options()).clone();
    let single = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("connect single-connection pool to the same test database");

    if !age::is_available(&single).await {
        eprintln!("AGE unavailable in this database — skipping search_path leak check");
        return;
    }

    let baseline: String = sqlx::query_scalar("SHOW search_path")
        .fetch_one(&single)
        .await
        .unwrap();
    assert!(
        !baseline.contains("ag_catalog"),
        "test setup invariant: search_path must not already contain ag_catalog, got: {baseline}"
    );

    age::cypher(&single, "RETURN 1").await.unwrap();

    // max_connections(1) guarantees this reuses the very connection
    // `cypher()` just acquired and released.
    let after: String = sqlx::query_scalar("SHOW search_path")
        .fetch_one(&single)
        .await
        .unwrap();
    assert!(
        !after.contains("ag_catalog"),
        "search_path leaked out of cypher()'s transaction onto the pooled connection: {after}"
    );
    assert_eq!(
        after, baseline,
        "search_path after cypher() must match the pre-call baseline exactly"
    );
}
