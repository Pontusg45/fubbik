mod common;
use axum::http::StatusCode;
use common::{TestApp, TestUser};
use serde_json::{Value, json};

fn manifest() -> Value {
    json!({ "version": 1, "project": "sample", "language": "java", "extractor": "fixture-v1",
        "complete": true, "symbols": [
            { "key": "sample.Lookup#find(int)", "title": "find(int)", "signature": "String find(int id)",
              "documentation": "Find by ID", "path": "src/Lookup.java", "line": 5 },
            { "key": "sample.Lookup#find(java.lang.String)", "title": "find(String)", "signature": "String find(String name)",
              "documentation": "Find by name", "path": "src/Lookup.java", "line": 9 }
        ] })
}

async fn setup(pool: sqlx::PgPool) -> (TestApp, TestUser, String) {
    let app = TestApp::new(pool);
    let user = app.signup("source@example.test", "Source").await;
    let response = app
        .post(
            &user,
            "/api/spaces",
            json!({"name":"source", "kind":"code"}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let space: Value = TestApp::json(response).await;
    (app, user, space["id"].as_str().unwrap().into())
}

async fn import(app: &TestApp, user: &TestUser, space: &str, manifest: Value) -> Value {
    let response = app
        .post(
            user,
            "/api/documents/import-source",
            json!({"spaceId": space,"manifest":manifest}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    TestApp::json(response).await
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn stable_symbols_sync_and_preserve_human_edits(pool: sqlx::PgPool) {
    let (app, user, space) = setup(pool.clone()).await;
    let first = import(&app, &user, &space, manifest()).await;
    assert_eq!(first["created"], 2);
    let again = import(&app, &user, &space, manifest()).await;
    assert_eq!(again["documentId"], first["documentId"]);
    assert_eq!(again["unchanged"], 2);
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT chunk_id FROM source_documentation_symbol ORDER BY symbol_key")
            .fetch_all(&pool)
            .await
            .unwrap();
    let mut changed = manifest();
    changed["symbols"][0]["line"] = json!(20);
    changed["symbols"][0]["documentation"] = json!("New docs");
    assert_eq!(
        import(&app, &user, &space, changed.clone()).await["updated"],
        1
    );
    let ids_after: Vec<String> =
        sqlx::query_scalar("SELECT chunk_id FROM source_documentation_symbol ORDER BY symbol_key")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(ids, ids_after);
    sqlx::query("UPDATE chunk SET content='My annotation' WHERE id=$1")
        .bind(&ids[0])
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        import(&app, &user, &space, changed).await["conflicts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let content: String = sqlx::query_scalar("SELECT content FROM chunk WHERE id=$1")
        .bind(&ids[0])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(content, "My annotation");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn partial_scans_do_not_remove_symbols_and_complete_scans_restore_them(pool: sqlx::PgPool) {
    let (app, user, space) = setup(pool).await;
    import(&app, &user, &space, manifest()).await;
    let mut empty = manifest();
    empty["symbols"] = json!([]);
    empty["complete"] = json!(false);
    assert_eq!(
        import(&app, &user, &space, empty.clone()).await["missing"],
        0
    );
    empty["complete"] = json!(true);
    assert_eq!(import(&app, &user, &space, empty).await["missing"], 2);
    assert_eq!(import(&app, &user, &space, manifest()).await["updated"], 2);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn rejects_foreign_spaces_and_duplicate_symbols(pool: sqlx::PgPool) {
    let (app, user, space) = setup(pool).await;
    let other = app.signup("other@example.test", "Other").await;
    let response = app
        .post(
            &other,
            "/api/documents/import-source",
            json!({"spaceId":space,"manifest":manifest()}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let mut duplicate = manifest();
    duplicate["symbols"][1] = duplicate["symbols"][0].clone();
    let response = app
        .post(
            &user,
            "/api/documents/import-source",
            json!({"spaceId":space,"manifest":duplicate}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn late_failure_rolls_back_document_chunks_and_mapping(pool: sqlx::PgPool) {
    let (app, user, space) = setup(pool.clone()).await;
    sqlx::raw_sql("CREATE FUNCTION reject_source_ref() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'forced failure'; END; $$; CREATE TRIGGER reject_source_ref BEFORE INSERT ON chunk_file_ref FOR EACH ROW EXECUTE FUNCTION reject_source_ref();")
        .execute(&pool).await.unwrap();
    let response = app
        .post(
            &user,
            "/api/documents/import-source",
            json!({"spaceId":space,"manifest":manifest()}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM document WHERE source_path LIKE 'source-docs://%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk WHERE title LIKE 'find%'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn new_symbols_reorder_without_slot_collisions_and_missing_symbols_leave_render(
    pool: sqlx::PgPool,
) {
    let (app, user, space) = setup(pool.clone()).await;
    let first = import(&app, &user, &space, manifest()).await;
    let document_id = first["documentId"].as_str().unwrap();
    let mut extended = manifest();
    let mut added = extended["symbols"][0].clone();
    added["key"] = json!("sample.Lookup#aaa()");
    added["title"] = json!("aaa");
    extended["symbols"].as_array_mut().unwrap().push(added);
    assert_eq!(import(&app, &user, &space, extended).await["created"], 1);
    assert_eq!(import(&app, &user, &space, manifest()).await["missing"], 1);
    let rendered = TestApp::json(
        app.get(&user, &format!("/api/documents/{document_id}/render"))
            .await,
    )
    .await;
    assert!(!rendered["markdown"].as_str().unwrap().contains("## aaa"));
    let response = app
        .post(
            &user,
            &format!("/api/documents/{document_id}/sync"),
            json!({"content":"# overwritten"}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn concurrent_first_imports_share_one_document(pool: sqlx::PgPool) {
    let (app, user, space) = setup(pool.clone()).await;
    let (first, second) = tokio::join!(
        import(&app, &user, &space, manifest()),
        import(&app, &user, &space, manifest())
    );
    assert_eq!(first["documentId"], second["documentId"]);
    assert_eq!(
        first["created"].as_u64().unwrap() + second["created"].as_u64().unwrap(),
        2
    );
}
