use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn dev_state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState { pool, implicit_dev_session: true }
}

async fn seed_dev_user(pool: &sqlx::PgPool) {
    fubbik_db::repo::user::create(pool, "dev@fubbik.local", "Dev", None)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_then_fetch_chunk(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"Naming","content":"kebab-case"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["type"], "note", "type must default to note");

    let res = app
        .oneshot(Request::get(format!("/api/chunks/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn missing_chunk_is_404(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(Request::get("/api/chunks/nonexistent").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_request_is_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(fubbik_api::AppState { pool, implicit_dev_session: false });

    let res = app
        .oneshot(Request::get("/api/chunks").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn blank_title_is_400(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"   "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
