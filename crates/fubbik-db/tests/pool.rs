#[sqlx::test]
async fn migrations_create_chunk_table(pool: sqlx::PgPool) {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk")
        .fetch_one(&pool)
        .await
        .expect("chunk table exists");
    assert_eq!(count, 0);
}
