use fubbik_db::seed::seed_core;
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
