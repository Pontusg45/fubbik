use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState { pool, implicit_dev_session: false }
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn signup_then_signin_sets_cookie(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let signup = app
        .clone()
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"a@b.test","password":"hunter22","name":"Alice"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(signup.status(), StatusCode::OK);

    let signin = app
        .oneshot(
            Request::post("/api/auth/sign-in/email")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"email":"a@b.test","password":"hunter22"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(signin.status(), StatusCode::OK);
    assert!(
        signin.headers().get("set-cookie").is_some(),
        "sign-in must set a session cookie"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn wrong_password_is_unauthorized(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    app.clone()
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"a@b.test","password":"hunter22","name":"Alice"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let res = app
        .oneshot(
            Request::post("/api/auth/sign-in/email")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"email":"a@b.test","password":"wrong"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
