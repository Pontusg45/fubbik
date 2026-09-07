mod common;

use axum::http::StatusCode;
use common::{TestApp, TestUser};

async fn create_chunk(
    app: &TestApp,
    user: &TestUser,
    title: &str,
    chunk_type: &str,
    tags: &[&str],
) -> String {
    let response = app
        .post(
            user,
            "/api/chunks",
            serde_json::json!({ "title": title, "type": chunk_type, "tags": tags }),
        )
        .await;
    let status = response.status();
    let body = TestApp::json(response).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body}");
    body["id"].as_str().unwrap().to_owned()
}

async fn create_ai_chunk(
    app: &TestApp,
    user: &TestUser,
    title: &str,
    chunk_type: &str,
    tags: &[&str],
) -> String {
    let response = app
        .post(
            user,
            "/api/chunks",
            serde_json::json!({ "title": title, "type": chunk_type, "tags": tags, "origin": "ai" }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    TestApp::json(response).await["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

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

async fn create_scoped_chunk(
    app: &TestApp,
    user: &TestUser,
    title: &str,
    space_id: &str,
) -> String {
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
async fn groups_and_pages_chunks_by_type(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let owner = app.signup("groups@example.com", "Owner").await;
    create_chunk(&app, &owner, "First note", "note", &[]).await;
    let second = create_chunk(&app, &owner, "Second note", "note", &[]).await;
    create_chunk(&app, &owner, "Guide", "guide", &[]).await;

    let response = app.get(&owner, "/api/chunks/grouped?groupBy=type").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = TestApp::json(response).await;
    assert_eq!(body["totalGroups"], 2);
    let mut groups = body["groups"].as_array().unwrap().clone();
    groups.sort_by_key(|group| group["groupName"].as_str().unwrap().to_owned());
    assert_eq!(
        groups[0],
        serde_json::json!({ "groupName": "guide", "count": 1 })
    );
    assert_eq!(
        groups[1],
        serde_json::json!({ "groupName": "note", "count": 2 })
    );

    let response = app
        .get(
            &owner,
            "/api/chunks/grouped/note/chunks?groupBy=type&limit=1&offset=0",
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = TestApp::json(response).await;
    assert_eq!(body["total"], 2);
    assert_eq!(body["chunks"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["chunks"][0]["id"], second,
        "group pages newest updates first"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn compound_tag_groups_are_scoped_and_split_by_origin(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let owner = app.signup("compound-groups@example.com", "Owner").await;
    let outsider = app
        .signup("compound-groups-other@example.com", "Other")
        .await;
    create_chunk(&app, &owner, "Human", "note", &["alpha", "beta"]).await;
    create_ai_chunk(&app, &owner, "Agent", "guide", &["alpha"]).await;
    create_chunk(&app, &outsider, "Hidden", "note", &["alpha"]).await;

    let response = app
        .get(
            &owner,
            "/api/chunks/grouped?groupBy=tagtype&subGroupBy=origin",
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = TestApp::json(response).await;
    let groups = body["groups"].as_array().unwrap();
    let alpha = groups
        .iter()
        .find(|group| group["groupName"] == "alpha")
        .unwrap();
    assert_eq!(alpha["count"], 2);
    assert_eq!(alpha["subGroups"].as_array().unwrap().len(), 2);
    let beta = groups
        .iter()
        .find(|group| group["groupName"] == "beta")
        .unwrap();
    assert_eq!(beta["count"], 1);
    assert_eq!(
        beta["subGroups"],
        serde_json::json!([{ "groupName": "human", "count": 1 }])
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn space_grouping_includes_global_chunks_and_global_filter_excludes_scoped_chunks(
    pool: sqlx::PgPool,
) {
    let app = TestApp::new(pool);
    let owner = app.signup("space-groups@example.com", "Owner").await;
    let space_id = create_space(&app, &owner, "Platform").await;
    create_scoped_chunk(&app, &owner, "Scoped", &space_id).await;
    create_chunk(&app, &owner, "Global", "note", &[]).await;

    let response = app
        .get(
            &owner,
            &format!("/api/chunks/grouped?groupBy=type&spaceId={space_id}"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(TestApp::json(response).await["groups"][0]["count"], 2);

    let response = app
        .get(&owner, "/api/chunks/grouped?groupBy=type&global=true")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(TestApp::json(response).await["groups"][0]["count"], 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn all_tag_mode_requires_every_requested_tag(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let owner = app.signup("tag-filter-groups@example.com", "Owner").await;
    create_chunk(&app, &owner, "Both", "note", &["alpha", "beta"]).await;
    create_chunk(&app, &owner, "One", "note", &["alpha"]).await;

    let response = app
        .get(
            &owner,
            "/api/chunks/grouped?groupBy=type&tags=alpha,beta&tagMode=all",
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(TestApp::json(response).await["groups"][0]["count"], 1);
}
