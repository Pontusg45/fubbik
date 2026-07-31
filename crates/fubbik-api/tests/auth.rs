use axum::body::Body;
use axum::http::{Request, StatusCode};
use fubbik_api::auth::session::{COOKIE_NAME, DEV_EMAIL};
use fubbik_db::repo::user;
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState { pool, implicit_dev_session: false }
}

fn state_dev(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState { pool, implicit_dev_session: true }
}

/// Extracts just the `name=value` pair from a response's `set-cookie`
/// header, suitable for replaying as a request's `cookie` header — the same
/// thing a real cookie jar does when it drops Path/HttpOnly/SameSite/Max-Age
/// before sending a cookie back on the next request.
fn cookie_pair(response: &axum::response::Response) -> String {
    let raw = response
        .headers()
        .get("set-cookie")
        .expect("response should set a cookie")
        .to_str()
        .unwrap();
    raw.split(';').next().unwrap().to_string()
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
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

/// Sequential duplicate sign-up: exercises the `find_by_email` pre-check
/// path in `sign_up` and confirms the user-facing result is a clean 409.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn duplicate_signup_is_conflict(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let body = || {
        Body::from(r#"{"email":"dup@b.test","password":"hunter22","name":"Dup"}"#)
    };

    let first = app
        .clone()
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(body())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(body())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CONFLICT);
}

/// Fires two sign-ups for the same brand-new email concurrently, racing
/// past the `find_by_email` pre-check into the `user_email_unique`
/// constraint. Whichever request loses the race must still come back as a
/// clean 409, never a 500 — regardless of which code path (pre-check or
/// DB-constraint mapping) actually catches it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn concurrent_duplicate_signup_yields_conflict_not_500(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let body = || {
        Body::from(r#"{"email":"race@b.test","password":"hunter22","name":"Racer"}"#)
    };

    let app1 = app.clone();
    let app2 = app.clone();

    let (r1, r2) = tokio::join!(
        app1.oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(body())
                .unwrap()
        ),
        app2.oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(body())
                .unwrap()
        )
    );

    let statuses = [r1.unwrap().status(), r2.unwrap().status()];
    assert!(
        statuses.contains(&StatusCode::OK),
        "one concurrent signup should succeed: {statuses:?}"
    );
    assert!(
        statuses.contains(&StatusCode::CONFLICT),
        "the other should be a clean 409 conflict, got {statuses:?}"
    );
    assert!(
        !statuses.contains(&StatusCode::INTERNAL_SERVER_ERROR),
        "duplicate signup must never 500, got {statuses:?}"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_session_with_valid_cookie_returns_current_user(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let signup = app
        .clone()
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"session@b.test","password":"hunter22","name":"Sess"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(signup.status(), StatusCode::OK);
    let cookie = cookie_pair(&signup);

    let res = app
        .oneshot(
            Request::get("/api/auth/get-session")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let json = json_body(res).await;
    assert_eq!(json["email"], "session@b.test");
    assert_eq!(json["name"], "Sess");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_session_without_cookie_is_unauthorized(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .oneshot(
            Request::get("/api/auth/get-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_session_with_garbage_cookie_is_unauthorized_not_500(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .oneshot(
            Request::get("/api/auth/get-session")
                .header("cookie", format!("{COOKIE_NAME}=not-a-real-token"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_session_falls_back_to_dev_user_when_no_cookie(pool: sqlx::PgPool) {
    user::create(&pool, DEV_EMAIL, "Dev User", None)
        .await
        .unwrap();

    let app = fubbik_api::router(state_dev(pool));
    let res = app
        .oneshot(
            Request::get("/api/auth/get-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let json = json_body(res).await;
    assert_eq!(json["email"], DEV_EMAIL);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_session_dev_fallback_401s_when_dev_user_missing(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state_dev(pool));
    let res = app
        .oneshot(
            Request::get("/api/auth/get-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn signed_out_session_cookie_is_not_replayable(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    app.clone()
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"logout@b.test","password":"hunter22","name":"Out"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let signin = app
        .clone()
        .oneshot(
            Request::post("/api/auth/sign-in/email")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"email":"logout@b.test","password":"hunter22"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(signin.status(), StatusCode::OK);
    let cookie = cookie_pair(&signin);

    let signout = app
        .clone()
        .oneshot(
            Request::post("/api/auth/sign-out")
                .header("cookie", cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(signout.status(), StatusCode::OK);

    let reuse = app
        .oneshot(
            Request::get("/api/auth/get-session")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        reuse.status(),
        StatusCode::UNAUTHORIZED,
        "signed-out session token must not be replayable"
    );
}
