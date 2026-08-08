//! HTTP-level tests for the `settings` domain: three key-value scopes
//! (`user`, `codebase`, `instance`) plus the computed `features` view.
//!
//! Repo-level ownership guards for `codebase_settings` (the `EXISTS`
//! clauses on both read and write) are proven load-bearing in
//! `fubbik-db/tests/settings.rs` by removing them and watching a named
//! test fail. This file covers the HTTP surface: the seven routes'
//! response shapes and status codes, the service-level ownership
//! pre-check that turns a foreign `spaceId` into a clean 404 (proven
//! load-bearing below the same way), and — the part this domain exists to
//! get right — that `GET /settings/features` and `GET /settings/instance`
//! really are reachable with **no session at all**, while every other
//! route in this domain 401s without one.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
    }
}

async fn signup(app: axum::Router, email: &str, name: &str) -> String {
    let res = app
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"email":"{email}","password":"hunter22","name":"{name}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "signup must succeed");
    res.headers()
        .get("set-cookie")
        .expect("signup should set a session cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    if body.is_empty() {
        return serde_json::Value::Null;
    }
    serde_json::from_slice(&body).unwrap()
}

async fn user_id_for_email(pool: &sqlx::PgPool, email: &str) -> String {
    sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = $1"#, email)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn seed_space(pool: &sqlx::PgPool, user_id: &str, name: &str) -> String {
    fubbik_db::repo::space::create(
        pool,
        user_id,
        fubbik_db::repo::space::NewSpace {
            name: name.into(),
            kind: "wiki".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id
}

async fn get(app: axum::Router, path: &str, cookie: Option<&str>) -> axum::response::Response {
    let mut req = Request::get(path);
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap()
}

async fn patch(
    app: axum::Router,
    path: &str,
    cookie: Option<&str>,
    body: serde_json::Value,
) -> axum::response::Response {
    let mut req = Request::patch(path).header("content-type", "application/json");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    app.oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

// --- /settings/features ---

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn features_is_reachable_with_no_session_and_defaults_all_true(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = get(app, "/api/settings/features", None).await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "GET /settings/features must not require a session"
    );
    let body = json_body(res).await;
    assert_eq!(
        body,
        serde_json::json!({
            "aiEnabled": true,
            "enrichmentEnabled": true,
            "semanticSearchEnabled": true,
            "aiSuggestionsEnabled": true,
            "vocabularySuggestEnabled": true
        }),
        "all five flags must default to true when instance_settings is empty"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn features_reflects_stored_instance_setting_and_leaves_others_defaulted(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-features@b.test", "Alice").await;

    patch(
        app.clone(),
        "/api/settings/instance",
        Some(&cookie),
        serde_json::json!({ "key": "aiEnabled", "value": false }),
    )
    .await;

    let res = get(app, "/api/settings/features", None).await;
    let body = json_body(res).await;
    assert_eq!(body["aiEnabled"], false);
    assert_eq!(
        body["enrichmentEnabled"], true,
        "an unset flag must still default to true"
    );
}

// --- /settings/user ---

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn user_settings_require_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = get(app.clone(), "/api/settings/user", None).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = patch(
        app,
        "/api/settings/user",
        None,
        serde_json::json!({ "key": "theme", "value": "dark" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn user_settings_are_isolated_per_user(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-user@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-user@b.test", "Bob").await;

    let res = patch(
        app.clone(),
        "/api/settings/user",
        Some(&alice_cookie),
        serde_json::json!({ "key": "theme", "value": "dark" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({"message": "Updated"})
    );

    let alice_view =
        json_body(get(app.clone(), "/api/settings/user", Some(&alice_cookie)).await).await;
    assert_eq!(alice_view, serde_json::json!({ "theme": "dark" }));

    let bob_view = json_body(get(app.clone(), "/api/settings/user", Some(&bob_cookie)).await).await;
    assert_eq!(
        bob_view,
        serde_json::json!({}),
        "Bob must not see Alice's user settings"
    );
}

// --- /settings/codebase ---

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn codebase_settings_require_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = get(app.clone(), "/api/settings/codebase?codebaseId=x", None).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = patch(
        app,
        "/api/settings/codebase",
        None,
        serde_json::json!({ "codebaseId": "x", "key": "k", "value": "v" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn codebase_settings_round_trip_for_own_space(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-cb@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-cb@b.test").await;
    let space_id = seed_space(&pool, &user_id, "alices-space").await;

    let res = patch(
        app.clone(),
        "/api/settings/codebase",
        Some(&cookie),
        serde_json::json!({ "codebaseId": space_id, "key": "defaultChunkType", "value": "note" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({"message": "Updated"})
    );

    let res = get(
        app,
        &format!("/api/settings/codebase?codebaseId={space_id}"),
        Some(&cookie),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({ "defaultChunkType": "note" })
    );
}

/// The escalated, brief-mandated behaviour: writing a setting on another
/// user's space must be impossible. 404, and Alice's space must have
/// gained nothing from Bob's rejected write — a status-only assertion
/// would pass even if the write partially mutated something.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn set_codebase_setting_for_another_users_space_is_404_and_leaves_it_unchanged(
    pool: sqlx::PgPool,
) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cb-x@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cb-x@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-cb-x@b.test").await;
    let alices_space = seed_space(&pool, &alice_id, "alices-space").await;

    let res = patch(
        app.clone(),
        "/api/settings/codebase",
        Some(&bob_cookie),
        serde_json::json!({ "codebaseId": alices_space, "key": "defaultChunkType", "value": "hijacked" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let view = json_body(
        get(
            app,
            &format!("/api/settings/codebase?codebaseId={alices_space}"),
            Some(&alice_cookie),
        )
        .await,
    )
    .await;
    assert_eq!(
        view,
        serde_json::json!({}),
        "Alice's space must have gained no setting from Bob's rejected write"
    );
}

/// Same escalated behaviour, read side: another user cannot even see that
/// a space has settings, they must get a clean 404, not an empty `{}`
/// that leaks the space's existence-with-no-settings state.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_codebase_settings_for_another_users_space_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let bob_cookie = signup(app.clone(), "bob-cb-r@b.test", "Bob").await;
    let alice_id = {
        signup(app.clone(), "alice-cb-r@b.test", "Alice").await;
        user_id_for_email(&pool, "alice-cb-r@b.test").await
    };
    let alices_space = seed_space(&pool, &alice_id, "alices-space").await;

    let res = get(
        app,
        &format!("/api/settings/codebase?codebaseId={alices_space}"),
        Some(&bob_cookie),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn codebase_settings_for_nonexistent_space_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cb-none@b.test", "Alice").await;

    let res = get(
        app,
        "/api/settings/codebase?codebaseId=no-such-space",
        Some(&cookie),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// --- /settings/instance ---

/// The core divergence-from-the-rest-of-the-domain behaviour: `GET
/// /settings/instance` needs no session, but `PATCH` does. This is a
/// faithful port of Node's own asymmetric guarding
/// (`packages/api/src/settings/routes.ts:55-71`), not a bug.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn instance_get_is_open_but_patch_requires_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = get(app.clone(), "/api/settings/instance", None).await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "GET /settings/instance must not require a session"
    );
    assert_eq!(json_body(res).await, serde_json::json!({}));

    let res = patch(
        app,
        "/api/settings/instance",
        None,
        serde_json::json!({ "key": "aiEnabled", "value": false }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "PATCH /settings/instance must require a session"
    );
}

/// No admin/role check: any authenticated user can flip a global instance
/// flag, and it takes effect instance-wide (visible to a second,
/// unrelated user's request and to the unauthenticated `GET`). This
/// asserts the deliberate behaviour so a future change that adds gating
/// breaks this test rather than passing silently.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn any_authenticated_user_can_write_instance_settings_and_it_is_visible_globally(
    pool: sqlx::PgPool,
) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-inst@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-inst@b.test", "Bob").await;

    let res = patch(
        app.clone(),
        "/api/settings/instance",
        Some(&alice_cookie),
        serde_json::json!({ "key": "aiEnabled", "value": false }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    // Bob (a different, unrelated authenticated user) sees Alice's write.
    let bob_view =
        json_body(get(app.clone(), "/api/settings/instance", Some(&bob_cookie)).await).await;
    assert_eq!(bob_view, serde_json::json!({ "aiEnabled": false }));

    // So does a fully unauthenticated caller.
    let anon_view = json_body(get(app, "/api/settings/instance", None).await).await;
    assert_eq!(anon_view, serde_json::json!({ "aiEnabled": false }));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn instance_setting_upsert_overwrites_via_http(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-inst-up@b.test", "Alice").await;

    patch(
        app.clone(),
        "/api/settings/instance",
        Some(&cookie),
        serde_json::json!({ "key": "aiEnabled", "value": true }),
    )
    .await;
    patch(
        app.clone(),
        "/api/settings/instance",
        Some(&cookie),
        serde_json::json!({ "key": "aiEnabled", "value": false }),
    )
    .await;

    let view = json_body(get(app, "/api/settings/instance", Some(&cookie)).await).await;
    assert_eq!(view, serde_json::json!({ "aiEnabled": false }));
}
