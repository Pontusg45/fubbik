//! Proves the axum extractor accepts better-auth's HMAC-signed session
//! cookie alongside Rust's own `fubbik_session` — see
//! `fubbik_api::auth::session::CurrentUser` and
//! `fubbik_api::auth::better_auth_cookie::verify`.
//!
//! The session row always holds the RAW token (`WHERE s.token = $1`); the
//! cookie better-auth actually sends carries `${rawToken}.${signature}`.
//! `TOKEN`/`SIG`/`SECRET` below are the exact fixture better-auth's own
//! signer would produce, reused from `better_auth_cookie`'s unit tests.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use fubbik_api::AppState;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

const SECRET: &str = "test-secret-value-at-least-32-chars-long-000000";
const TOKEN: &str = "AbCdEfGhIjKlMnOpQrStUvWxYz012345";
const SIG: &str = "OyhRBnvMlgxzHjKlrRsQqjXwtDV99cAmrGDWxOTAkzU=";

async fn seed_user(pool: &PgPool, email: &str) -> String {
    let user = fubbik_db::repo::user::create(pool, email, "Test User", None)
        .await
        .unwrap();
    user.id
}

/// Inserts a session row holding the RAW token directly — bypassing
/// `fubbik_db::repo::session::create`, which generates its own random
/// token and can't be pinned to the fixture value the signed cookie must
/// match.
async fn seed_session(pool: &PgPool, user_id: &str, token: &str) {
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO session (id, token, user_id, expires_at, created_at, updated_at)
           VALUES ($1, $2, $3, now() + interval '1 day', now(), now())"#,
        id,
        token,
        user_id,
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn test_app_with_secret(pool: PgPool, secret: &str) -> axum::Router {
    fubbik_api::router(AppState {
        pool,
        implicit_dev_session: false,
        better_auth_secret: secret.to_string(),
    })
}

async fn get_with_cookie(app: &axum::Router, path: &str, cookie: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::get(path)
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_better_auth_cookie_authenticates(pool: PgPool) {
    let user_id = seed_user(&pool, "a@b.test").await;
    // The session row holds the RAW token; the cookie carries token.signature.
    seed_session(&pool, &user_id, TOKEN).await;
    let app = test_app_with_secret(pool.clone(), SECRET).await;

    let res = get_with_cookie(
        &app,
        "/api/chunks",
        &format!("better-auth.session_token={TOKEN}.{SIG}"),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "a validly-signed better-auth cookie must authenticate"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_forged_signature_is_rejected_end_to_end(pool: PgPool) {
    let user_id = seed_user(&pool, "a@b.test").await;
    seed_session(&pool, &user_id, TOKEN).await;
    let app = test_app_with_secret(pool.clone(), SECRET).await;

    let res = get_with_cookie(
        &app,
        "/api/chunks",
        &format!("better-auth.session_token={TOKEN}.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "an unsigned or forged token must not reach the database"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn the_secure_prefixed_cookie_name_is_accepted(pool: PgPool) {
    let user_id = seed_user(&pool, "a@b.test").await;
    seed_session(&pool, &user_id, TOKEN).await;
    let app = test_app_with_secret(pool.clone(), SECRET).await;

    let res = get_with_cookie(
        &app,
        "/api/chunks",
        &format!("__Secure-better-auth.session_token={TOKEN}.{SIG}"),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "HTTPS deployments get the __Secure- prefix"
    );
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Sanity check on top of the bare 200s above: the resolved user really is
/// the one the session row points at, not e.g. the dev-session fallback
/// masking a broken lookup.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_better_auth_cookie_resolves_the_owning_user(pool: PgPool) {
    let user_id = seed_user(&pool, "owner@b.test").await;
    seed_session(&pool, &user_id, TOKEN).await;
    let app = test_app_with_secret(pool.clone(), SECRET).await;

    let res = get_with_cookie(
        &app,
        "/api/auth/get-session",
        &format!("better-auth.session_token={TOKEN}.{SIG}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let json = json_body(res).await;
    assert_eq!(json["email"], "owner@b.test");
}
