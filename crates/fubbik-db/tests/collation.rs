//! Proves that a freshly-provisioned test database orders text the way the
//! reference Node backend does. The Node backend runs on Homebrew Postgres,
//! which uses the ICU locale provider; ICU ignores punctuation at the
//! primary comparison level. A database created with the libc provider
//! instead compares raw bytes, so these two pairs sort in opposite order
//! between the two providers — they are exactly where the two disagree, so
//! this test fails loudly if the provider regresses.
//!
//! `#[sqlx::test]` provisions its database from a template cloned off
//! whatever cluster `DATABASE_URL` points at, so this also proves that
//! `#[sqlx::test]` databases inherit the cluster's locale provider rather
//! than defaulting to libc independently.

#[sqlx::test]
async fn orders_titles_the_way_icu_does(pool: sqlx::PgPool) {
    // Given the inline inputs and test fixtures.
    // When
    let provider: String = sqlx::query_scalar(
        "SELECT datlocprovider::text FROM pg_database WHERE datname = current_database()",
    )
    .fetch_one(&pool)
    .await
    .expect("pg_database is always readable");
    // Then
    assert_eq!(
        provider, "i",
        "test database's locale provider is not ICU — #[sqlx::test] databases do not \
         inherit the cluster's ICU provider, or the cluster itself was not initialized \
         with --locale-provider=icu. This means test databases and production disagree \
         on text ordering."
    );

    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT title FROM (VALUES \
            ('Catalog tables: x'), \
            ('Catalog-driven y'), \
            ('Chunk scope'), \
            ('Chunks as') \
         ) AS t(title) ORDER BY title ASC",
    )
    .fetch_all(&pool)
    .await
    .expect("literal VALUES ordering query succeeds");

    let catalog_tables = rows.iter().position(|r| r == "Catalog tables: x").unwrap();
    let catalog_driven = rows.iter().position(|r| r == "Catalog-driven y").unwrap();
    assert!(
        catalog_tables < catalog_driven,
        "\"Catalog tables: x\" must sort before \"Catalog-driven y\" under ICU ordering, \
         got: {rows:?}"
    );

    // Verified directly against the live reference (Node/Homebrew Postgres,
    // ICU, database `fubbik`) with these exact literal strings: "Chunk
    // scope" sorts BEFORE "Chunks as" — the space in "Chunk scope" is a
    // lower primary weight than the "s" that immediately follows "Chunk" in
    // "Chunks as", so once the shared "Chunk" prefix is exhausted the
    // comparison resolves on that difference rather than continuing past it.
    let chunk_scope = rows.iter().position(|r| r == "Chunk scope").unwrap();
    let chunks_as = rows.iter().position(|r| r == "Chunks as").unwrap();
    assert!(
        chunk_scope < chunks_as,
        "\"Chunk scope\" must sort before \"Chunks as\" under ICU ordering, got: {rows:?}"
    );
}
