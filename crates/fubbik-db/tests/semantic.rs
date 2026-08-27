//! Vector-ranking tests. These run in CI: `.github/workflows/rust.yml:17`
//! uses `pgvector/pgvector:pg18`, so the `vector` extension is present even
//! though Ollama never is.

use fubbik_db::repo::user;

async fn seed_user(pool: &sqlx::PgPool) -> String {
    user::create(pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id
}

/// Variant of `seed_user` for tests that need two distinct users — the
/// bare `seed_user` hardcodes one email, so a second call would collide on
/// the unique constraint.
async fn seed_user_with_email(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "User", None).await.unwrap().id
}

/// A 768-dimension vector that is all zeros except one hot index. Cosine
/// distance between two such vectors is 0 when the indices match and 1 when
/// they differ, which makes expected orderings exact rather than
/// approximate.
fn one_hot(index: usize) -> String {
    let mut parts = vec!["0"; 768];
    parts[index] = "1";
    format!("[{}]", parts.join(","))
}

async fn seed_chunk_with_vector(
    pool: &sqlx::PgPool,
    user_id: &str,
    id: &str,
    title: &str,
    hot: usize,
) {
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id, embedding)
         VALUES ($1, $2, 'content', 'note', $3, $4::text::vector)",
    )
    .bind(id)
    .bind(title)
    .bind(user_id)
    .bind(one_hot(hot))
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn semantic_search_orders_by_similarity(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "far", "Far", 5).await;
    seed_chunk_with_vector(&pool, &user, "near", "Near", 0).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;

    let hits =
        fubbik_db::repo::semantic::semantic_search(&pool, &query, Some(&user), &[], None, 10)
            .await
            .unwrap();

    // Order, not membership: a test that only asserts both ids are present
    // passes with the ORDER BY deleted.
    assert_eq!(
        hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        vec!["near", "far"]
    );
    assert!(hits[0].similarity > 0.99, "got {}", hits[0].similarity);
    assert!(hits[1].similarity < 0.01, "got {}", hits[1].similarity);
}

#[sqlx::test]
async fn semantic_search_skips_chunks_without_an_embedding(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "has", "Has", 0).await;
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id)
         VALUES ('none', 'None', 'c', 'note', $1)",
    )
    .bind(&user)
    .execute(&pool)
    .await
    .unwrap();

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;
    let hits =
        fubbik_db::repo::semantic::semantic_search(&pool, &query, Some(&user), &[], None, 10)
            .await
            .unwrap();

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, "has");
}

/// `exclude` filters on `not_about @>`, dropping a chunk that would
/// otherwise rank *first* — so deleting the filter changes the result.
#[sqlx::test]
async fn semantic_search_excludes_terms_in_not_about(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "near", "Near", 0).await;
    seed_chunk_with_vector(&pool, &user, "far", "Far", 5).await;
    sqlx::query("UPDATE chunk SET not_about = '[\"billing\"]'::jsonb WHERE id = 'near'")
        .execute(&pool)
        .await
        .unwrap();

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;
    let hits = fubbik_db::repo::semantic::semantic_search(
        &pool,
        &query,
        Some(&user),
        &["billing".to_string()],
        None,
        10,
    )
    .await
    .unwrap();

    assert_eq!(
        hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        vec!["far"]
    );
}

#[sqlx::test]
async fn semantic_search_filters_by_scope(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "near", "Near", 0).await;
    seed_chunk_with_vector(&pool, &user, "far", "Far", 5).await;
    sqlx::query("UPDATE chunk SET scope = '{\"env\":\"prod\"}'::jsonb WHERE id = 'far'")
        .execute(&pool)
        .await
        .unwrap();

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;
    let hits = fubbik_db::repo::semantic::semantic_search(
        &pool,
        &query,
        Some(&user),
        &[],
        Some(&serde_json::json!({ "env": "prod" })),
        10,
    )
    .await
    .unwrap();

    assert_eq!(
        hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        vec!["far"]
    );
}

#[sqlx::test]
async fn semantic_search_is_scoped_to_the_user(pool: sqlx::PgPool) {
    let a = seed_user_with_email(&pool, "a@b.test").await;
    let b = seed_user_with_email(&pool, "b@c.test").await;
    seed_chunk_with_vector(&pool, &a, "mine", "Mine", 0).await;
    seed_chunk_with_vector(&pool, &b, "theirs", "Theirs", 0).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;
    let hits = fubbik_db::repo::semantic::semantic_search(&pool, &query, Some(&a), &[], None, 10)
        .await
        .unwrap();

    assert_eq!(
        hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        vec!["mine"]
    );
}

#[sqlx::test]
async fn find_neighbors_excludes_the_source_and_orders_by_distance(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "src", "Source", 0).await;
    seed_chunk_with_vector(&pool, &user, "near", "Near", 0).await;
    seed_chunk_with_vector(&pool, &user, "far", "Far", 7).await;

    let rows = fubbik_db::repo::semantic::find_neighbors_by_chunk_id(&pool, "src", &user, 10)
        .await
        .unwrap();

    assert_eq!(
        rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["near", "far"],
        "the source chunk must not be its own neighbour"
    );
    assert!(rows[0].distance < rows[1].distance);
}

/// Node's query carries `AND c.archived_at IS NULL`
/// (`packages/db/src/repository/semantic.ts:41`). Semantic search itself
/// does not — the asymmetry is Node's and is preserved.
#[sqlx::test]
async fn find_neighbors_skips_archived_chunks(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "src", "Source", 0).await;
    seed_chunk_with_vector(&pool, &user, "gone", "Gone", 0).await;
    sqlx::query("UPDATE chunk SET archived_at = now() WHERE id = 'gone'")
        .execute(&pool)
        .await
        .unwrap();

    let rows = fubbik_db::repo::semantic::find_neighbors_by_chunk_id(&pool, "src", &user, 10)
        .await
        .unwrap();
    assert!(rows.is_empty());
}

#[sqlx::test]
async fn find_neighbors_is_empty_when_the_source_has_no_embedding(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id)
         VALUES ('src', 'Source', 'c', 'note', $1)",
    )
    .bind(&user)
    .execute(&pool)
    .await
    .unwrap();
    seed_chunk_with_vector(&pool, &user, "other", "Other", 0).await;

    let rows = fubbik_db::repo::semantic::find_neighbors_by_chunk_id(&pool, "src", &user, 10)
        .await
        .unwrap();
    assert!(
        rows.is_empty(),
        "the CTE yields no source row, so the join yields nothing"
    );
}

/// `user_id: None` is the global-search path (Node's `semanticSearch` also
/// takes an optional `userId` that is always passed by callers today, but
/// the branch is reachable through the public repository API — see
/// controller ruling in task-4 review round 1). Seeds two distinct users
/// with one vectorised chunk each and asserts both come back, in distance
/// order, proving the `$2::text IS NULL OR c.user_id = $2` branch actually
/// widens the search rather than silently filtering everyone out.
#[sqlx::test]
async fn semantic_search_with_no_user_id_searches_across_users(pool: sqlx::PgPool) {
    let a = seed_user_with_email(&pool, "a@b.test").await;
    let b = seed_user_with_email(&pool, "b@c.test").await;
    seed_chunk_with_vector(&pool, &a, "near", "Near", 0).await;
    seed_chunk_with_vector(&pool, &b, "far", "Far", 5).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;
    let hits = fubbik_db::repo::semantic::semantic_search(&pool, &query, None, &[], None, 10)
        .await
        .unwrap();

    assert_eq!(
        hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        vec!["near", "far"],
        "None should search across all users, ordered by similarity"
    );
}
