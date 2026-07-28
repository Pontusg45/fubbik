use fubbik_db::age;

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

    let matched = age::cypher(&pool, "MATCH (n:chunk) RETURN n").await.unwrap();
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
