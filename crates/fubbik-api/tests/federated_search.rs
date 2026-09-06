mod common;

use axum::http::StatusCode;
use common::{TestApp, TestUser};

async fn create_space(app: &TestApp, user: &TestUser, name: &str) -> String {
    let response = app
        .post(user, "/api/spaces", serde_json::json!({ "name": name }))
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    TestApp::json(response).await["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn create_chunk(app: &TestApp, user: &TestUser, title: &str, space_id: &str) -> String {
    let response = app
        .post(
            user,
            "/api/chunks",
            serde_json::json!({ "title": title, "spaceIds": [space_id] }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    TestApp::json(response).await["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn federated_search_returns_space_name_and_is_user_scoped(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let owner = app.signup("federated@example.com", "Owner").await;
    let outsider = app.signup("federated-other@example.com", "Other").await;
    let space_id = create_space(&app, &owner, "Platform").await;
    let other_space_id = create_space(&app, &outsider, "Hidden").await;
    let chunk_id = create_chunk(&app, &owner, "Federated authentication", &space_id).await;
    create_chunk(
        &app,
        &owner,
        "Federated authentication deployment notes",
        &space_id,
    )
    .await;
    create_chunk(
        &app,
        &outsider,
        "Federated authentication secret",
        &other_space_id,
    )
    .await;

    let response = app
        .get(
            &owner,
            "/api/chunks/search/federated?search=Federated%20authentication&limit=8",
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = TestApp::json(response).await;

    assert_eq!(body["total"], 2);
    assert_eq!(body["chunks"][0]["id"], chunk_id);
    assert_eq!(body["chunks"][0]["codebaseName"], "Platform");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn federated_search_enforces_its_fifty_row_cap(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let owner = app.signup("federated-cap@example.com", "Owner").await;
    let space_id = create_space(&app, &owner, "Platform").await;
    for index in 0..55 {
        create_chunk(&app, &owner, &format!("Chunk {index:02}"), &space_id).await;
    }

    let response = app
        .get(&owner, "/api/chunks/search/federated?limit=999")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = TestApp::json(response).await;
    assert_eq!(body["total"], 55);
    assert_eq!(body["chunks"].as_array().unwrap().len(), 50);
}
