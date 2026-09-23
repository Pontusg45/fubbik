mod common;

use axum::http::StatusCode;
use common::{TestApp, TestUser};

async fn create_chunk(app: &TestApp, user: &TestUser, title: &str) -> String {
    let response = app
        .post(user, "/api/chunks", serde_json::json!({ "title": title }))
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    TestApp::json(response).await["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn vector(x: f32, y: f32) -> String {
    let mut values = vec![0.0_f32; 768];
    values[0] = x;
    values[1] = y;
    format!(
        "[{}]",
        values
            .iter()
            .map(f32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

async fn set_embedding(pool: &sqlx::PgPool, id: &str, embedding: String, updated_at: &str) {
    sqlx::query(
        "UPDATE chunk SET embedding = $1::vector, updated_at = $2::timestamp WHERE id = $3",
    )
    .bind(embedding)
    .bind(updated_at)
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn clusters_use_recent_seeds_and_rank_vector_neighbors(pool: sqlx::PgPool) {
    // Given
    let app = TestApp::new(pool.clone());
    let owner = app.signup("clusters@example.com", "Owner").await;
    let seed = create_chunk(&app, &owner, "Seed").await;
    let close = create_chunk(&app, &owner, "Close").await;
    let far = create_chunk(&app, &owner, "Far").await;

    set_embedding(&pool, &seed, vector(1.0, 0.0), "2026-01-03 00:00:00").await;
    set_embedding(&pool, &close, vector(0.9, 0.1), "2026-01-02 00:00:00").await;
    set_embedding(&pool, &far, vector(0.0, 1.0), "2026-01-01 00:00:00").await;

    // When
    let response = app.get(&owner, "/api/chunks/clusters").await;
    // Then
    assert_eq!(response.status(), StatusCode::OK);
    let clusters = TestApp::json(response).await;
    let clusters = clusters.as_array().unwrap();

    assert_eq!(clusters.len(), 1, "members are skipped as later seeds");
    assert_eq!(clusters[0]["seedId"], seed);
    assert_eq!(clusters[0]["members"][0]["id"], close);
    assert!(clusters[0]["members"][0]["similarity"].as_f64().unwrap() > 0.9);
    assert_eq!(clusters[0]["members"][1]["id"], far);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn clusters_are_empty_without_embeddings(pool: sqlx::PgPool) {
    // Given
    let app = TestApp::new(pool);
    let owner = app.signup("clusters-empty@example.com", "Owner").await;
    create_chunk(&app, &owner, "No embedding").await;

    // When
    let response = app.get(&owner, "/api/chunks/clusters").await;
    // Then
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(TestApp::json(response).await, serde_json::json!([]));
}
