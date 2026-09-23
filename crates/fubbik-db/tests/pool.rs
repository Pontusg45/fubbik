#[sqlx::test]
async fn migrations_create_chunk_table(pool: sqlx::PgPool) {
    // Given the inline inputs and test fixtures.
    // When
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk")
        .fetch_one(&pool)
        .await
        .expect("chunk table exists");
    // Then
    assert_eq!(count, 0);
}
