//! Similarity-ranking tests for `find_similar_by_embedding`. These run in
//! CI: `.github/workflows/rust.yml:17` uses `pgvector/pgvector:pg18`, so the
//! `vector` extension is present even though Ollama never is.

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

/// The threshold must exclude, not merely rank. A test that checks only
/// that an above-threshold match comes back still passes with the filter
/// deleted entirely.
#[sqlx::test]
async fn find_similar_drops_matches_below_the_threshold(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "identical", "Identical", 0).await;
    seed_chunk_with_vector(&pool, &user, "orthogonal", "Orthogonal", 9).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;

    let hits =
        fubbik_db::repo::similarity::find_similar_by_embedding(&pool, &query, &user, None, 0.75, 5)
            .await
            .unwrap();

    assert_eq!(
        hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        vec!["identical"],
        "the orthogonal chunk has similarity 0 and must be filtered out"
    );
}

#[sqlx::test]
async fn find_similar_honours_exclude_id(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "self", "Self", 0).await;
    seed_chunk_with_vector(&pool, &user, "other", "Other", 0).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;

    let hits = fubbik_db::repo::similarity::find_similar_by_embedding(
        &pool,
        &query,
        &user,
        Some("self"),
        0.75,
        5,
    )
    .await
    .unwrap();

    assert_eq!(
        hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        vec!["other"]
    );
}

#[sqlx::test]
async fn find_similar_is_scoped_to_the_user(pool: sqlx::PgPool) {
    let a = seed_user_with_email(&pool, "a@b.test").await;
    let b = seed_user_with_email(&pool, "b@c.test").await;
    seed_chunk_with_vector(&pool, &a, "mine", "Mine", 0).await;
    seed_chunk_with_vector(&pool, &b, "theirs", "Theirs", 0).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;

    let hits =
        fubbik_db::repo::similarity::find_similar_by_embedding(&pool, &query, &a, None, 0.75, 5)
            .await
            .unwrap();

    assert_eq!(
        hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        vec!["mine"]
    );
}

/// Node applies `LIMIT` in SQL and *then* filters by threshold in JS
/// (`similarity.ts:78-81`), so a below-threshold row can consume a limit
/// slot and shrink the result below `LIMIT`, even when nothing else was
/// competing for that slot. This test pins that shrinkage: with 3 total
/// candidates and `LIMIT 3`, the below-threshold row `b` still gets fetched
/// (it's within the fetch window) and only then dropped, leaving 2 rows
/// rather than 3.
///
/// Note for the next reader: moving the filter into `WHERE` does **not**
/// make this test fail, and that is not a gap in the test — it is a
/// mathematical property of this specific query. The `WHERE` threshold and
/// the `ORDER BY` key are the same monotonic function of cosine distance
/// (`similarity = 1 - distance`), so filtering before `LIMIT` can only ever
/// remove candidates that would already sort *after* every retained
/// candidate. Filtering pre- or post-`LIMIT` therefore always yields
/// identical result sets for this query — there is no input that makes
/// them diverge. The comment on `find_similar_by_embedding` describing the
/// SQL move as "a behaviour change" is about structural fidelity to Node's
/// two-step shape, not about an observable difference in results; do not
/// try to "strengthen" this test to catch that mutation, it cannot be
/// done.
#[sqlx::test]
async fn find_similar_lets_a_below_threshold_row_consume_a_limit_slot(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "a", "A", 0).await;
    seed_chunk_with_vector(&pool, &user, "b", "B", 3).await;
    seed_chunk_with_vector(&pool, &user, "c", "C", 0).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;

    let hits =
        fubbik_db::repo::similarity::find_similar_by_embedding(&pool, &query, &user, None, 0.75, 3)
            .await
            .unwrap();

    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|h| h.id != "b"));
}
