//! Every error response — domain errors and axum's built-in extractor
//! rejections alike — must come back as `application/json` with the same
//! `{"message": ...}` shape. Measured before this fix: a bad query param, a
//! malformed JSON body, and a missing required field all came back as
//! `text/plain`; only domain errors (like a 404) were already JSON. A
//! client doing `res.json()` broke on three of those four classes.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn dev_state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: true,
    }
}

async fn seed_dev_user(pool: &sqlx::PgPool) {
    fubbik_db::repo::user::create(pool, "dev@localhost", "Dev", None)
        .await
        .unwrap();
}

/// Asserts the response has a `application/json` content-type and a body
/// of the shape `{"message": <non-empty string>}`, returning that message.
async fn assert_json_message_body(res: axum::response::Response) -> String {
    let content_type = res
        .headers()
        .get("content-type")
        .expect("error response must set a content-type header")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        content_type.starts_with("application/json"),
        "expected application/json, got: {content_type}"
    );

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body)
        .unwrap_or_else(|e| panic!("error body was not valid JSON ({e}): {body:?}"));

    let obj = json.as_object().expect("error body must be a JSON object");
    assert_eq!(
        obj.keys().collect::<Vec<_>>(),
        vec!["message"],
        "error body must have exactly the {{\"message\": ...}} shape, got: {json}"
    );
    let message = obj["message"].as_str().expect("message must be a string");
    assert!(!message.is_empty(), "message must not be empty");
    message.to_string()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn bad_query_param_is_json_400(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(
            Request::get("/api/chunks?sort=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_json_message_body(res).await;
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn malformed_json_body_is_json_400(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from("{not valid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_json_message_body(res).await;
}

/// A well-formed JSON body missing a required field (`title`) is a
/// distinct rejection path from malformed JSON — axum's default status for
/// this is 422, but the service layer's own validation failures (e.g. a
/// blank title) are already 400 via `AppError::Validation`. Both now go
/// through the same extractor, so both come back 400.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn missing_required_field_is_json_400(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"content":"no title field at all"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "missing required field must be 400, matching the service layer's own \
         validation failures (e.g. a blank title), not axum's default 422"
    );
    assert_json_message_body(res).await;
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn missing_content_type_is_json_415(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .body(Body::from(r#"{"title":"no content-type header"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "missing content-type is a distinct error class from a malformed \
         body — its existing, more specific status code is preserved"
    );
    assert_json_message_body(res).await;
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn domain_404_is_json(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(
            Request::get("/api/chunks/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let message = assert_json_message_body(res).await;
    assert_eq!(message, "chunk not found");
}
