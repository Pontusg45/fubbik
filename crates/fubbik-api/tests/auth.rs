use axum::body::Body;
use axum::http::{Request, StatusCode};
use fubbik_api::auth::session::COOKIE_NAME;
use fubbik_db::repo::user;
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
        better_auth_secret: "test-secret".into(),
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
        rate_limiter: Default::default(),
    }
}

fn state_dev(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: true,
        better_auth_secret: "test-secret".into(),
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
        rate_limiter: Default::default(),
    }
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
    let body = || Body::from(r#"{"email":"dup@b.test","password":"hunter22","name":"Dup"}"#);

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

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn signup_normalizes_email_and_signin_is_case_insensitive(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));

    let signup = app
        .clone()
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"Alice@Example.TEST","password":"hunter22","name":"Alice"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(signup.status(), StatusCode::OK);
    assert_eq!(
        json_body(signup).await["user"]["email"],
        "alice@example.test"
    );

    let stored_email: String =
        sqlx::query_scalar(r#"SELECT email FROM "user" WHERE email = 'alice@example.test'"#)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored_email, "alice@example.test");

    let signin = app
        .oneshot(
            Request::post("/api/auth/sign-in/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"ALICE@EXAMPLE.TEST","password":"hunter22"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(signin.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn signup_rejects_invalid_email_without_creating_a_user(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));

    let response = app
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"not-an-email","password":"hunter22","name":"Invalid"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let count: i64 = sqlx::query_scalar(r#"SELECT count(*) FROM "user""#)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn signin_rejects_invalid_email_as_bad_request(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let response = app
        .oneshot(
            Request::post("/api/auth/sign-in/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"not-an-email","password":"hunter22"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn signup_rejects_passwords_over_better_auths_128_character_limit(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let password = "x".repeat(129);
    let body = serde_json::json!({
        "email": "long-password@b.test",
        "password": password,
        "name": "Long"
    });

    let response = app
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        user::find_by_email(&pool, "long-password@b.test")
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn signup_measures_unicode_passwords_like_javascript(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    // 40 emoji are 80 UTF-16 code units (JavaScript's string.length), but
    // 160 UTF-8 bytes. Better Auth accepts this; a Rust byte-length check
    // would incorrectly reject it as over the 128-character limit.
    let password = "🦀".repeat(40);
    let body = serde_json::json!({
        "email": "unicode-password@b.test",
        "password": password,
        "name": "Unicode"
    });

    let response = app
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Fires two sign-ups for the same brand-new email concurrently, racing
/// past the `find_by_email` pre-check into the `user_email_unique`
/// constraint. Whichever request loses the race must still come back as a
/// clean 409, never a 500 — regardless of which code path (pre-check or
/// DB-constraint mapping) actually catches it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn concurrent_duplicate_signup_yields_conflict_not_500(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let body = || Body::from(r#"{"email":"race@b.test","password":"hunter22","name":"Racer"}"#);

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
    assert_eq!(json["user"]["email"], "session@b.test");
    assert_eq!(json["user"]["name"], "Sess");
    assert_eq!(json["session"]["userId"], json["user"]["id"]);
    assert!(json["session"]["expiresAt"].is_string());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_session_without_cookie_returns_null_like_better_auth(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .oneshot(
            Request::get("/api/auth/get-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await, serde_json::Value::Null);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_session_with_garbage_cookie_returns_null_not_500(pool: sqlx::PgPool) {
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
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await, serde_json::Value::Null);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_session_does_not_turn_implicit_dev_access_into_a_browser_session(pool: sqlx::PgPool) {
    // Seeded via the canonical bootstrap, not `user::create`: the fallback
    // now enforces the fixed `id = "dev-user"` invariant (matching Node's
    // `IMPLICIT_DEV_USER_ID`), so a same-email row under an arbitrary id —
    // what `user::create` would produce — is no longer an equivalent
    // fixture; it would collide with the bootstrap's own insert on the
    // `email` unique constraint, same failure mode Node's
    // `onConflictDoNothing({ target: user.id })` has for the same reason.
    user::ensure_implicit_dev_user(&pool).await.unwrap();

    let app = fubbik_api::router(state_dev(pool.clone()));
    let res = app
        .oneshot(
            Request::get("/api/auth/get-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    assert_eq!(json_body(res).await, serde_json::Value::Null);
}

/// The credential-session endpoint stays null for implicit dev access. API
/// extractors bootstrap that user lazily; polling from the browser must not.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_session_without_cookie_does_not_create_the_implicit_dev_user(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state_dev(pool.clone()));
    let res = app
        .oneshot(
            Request::get("/api/auth/get-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    assert_eq!(json_body(res).await, serde_json::Value::Null);
    assert!(user::find_by_id(&pool, "dev-user").await.unwrap().is_none());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn signup_and_signin_return_better_auth_compatible_envelopes(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let signup = app
        .clone()
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"shape@b.test","password":"hunter22","name":"Shape"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let signup_json = json_body(signup).await;
    assert!(signup_json["token"].is_string());
    assert_eq!(signup_json["user"]["email"], "shape@b.test");

    let signin = app
        .oneshot(
            Request::post("/api/auth/sign-in/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"shape@b.test","password":"hunter22"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let signin_json = json_body(signin).await;
    assert_eq!(signin_json["redirect"], false);
    assert!(signin_json["token"].is_string());
    assert_eq!(signin_json["user"]["email"], "shape@b.test");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn signin_accepts_a_better_auth_scrypt_hash_and_upgrades_it(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let user = user::create(&pool, "legacy@b.test", "Legacy", None)
        .await
        .unwrap();
    let account_id = fubbik_db::new_id();
    sqlx::query(
        r#"INSERT INTO account (id, account_id, provider_id, user_id, password, created_at, updated_at)
           VALUES ($1, $2, 'credential', $2, $3, now(), now())"#,
    )
    .bind(account_id)
    .bind(&user.id)
    .bind("00112233445566778899aabbccddeeff:c3b39f3eda79a45635ff935ee89c8c242531c4d6c6b5fe6bc27a369e3e1e16527bc69395cf710c41dcab0029263692fd327e358e9dc6bcdc7367f97f93ca44a0")
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::post("/api/auth/sign-in/email")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"legacy@b.test","password":"legacy-password"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let upgraded = user::find_by_id(&pool, &user.id).await.unwrap().unwrap();
    assert!(upgraded.password_hash.unwrap().starts_with("$argon2id$"));
    let legacy: Option<String> = sqlx::query_scalar(
        "SELECT password FROM account WHERE user_id = $1 AND provider_id = 'credential'",
    )
    .bind(&user.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(legacy, None, "the weaker legacy hash should be removed");
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
                .body(Body::from(
                    r#"{"email":"logout@b.test","password":"hunter22"}"#,
                ))
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
    assert_eq!(reuse.status(), StatusCode::OK);
    assert_eq!(
        json_body(reuse).await,
        serde_json::Value::Null,
        "signed-out session token must not be replayable"
    );
}
