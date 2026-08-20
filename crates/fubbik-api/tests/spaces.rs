//! HTTP-level tests for the `spaces` domain.
//!
//! Node's captured contract (`tests/fixtures/node-contract/spaces-*.json`,
//! `_questions.md`, `_mutating.md`) is the source of truth for three
//! deliberately different "space" shapes across GET endpoints:
//!
//! - `GET /api/spaces` (list) -> bare array of bare space objects, no
//!   `space_code_metadata` join at all.
//! - `GET /api/spaces/{id}` (detail) -> nested `{ space, code }`, NOT
//!   flattened.
//! - `GET /api/spaces/detect` on a match -> bare space object again (no
//!   `code` key); on no match -> a genuinely empty HTTP body.
//!
//! `chunk_space` is exercised at the repo level in `fubbik-db/tests/space.rs`
//! (no route in this slice exposes it directly); the cross-user tests here
//! cover the seven space routes themselves.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
        better_auth_secret: "test-secret".into(),
    }
}

/// Signs up a fresh user and returns the `name=value` session cookie pair
/// from the `set-cookie` response header, matching the pattern in
/// `tests/tags.rs::signup`.
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
    serde_json::from_slice(&body).unwrap()
}

async fn create_space(
    app: axum::Router,
    cookie: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/spaces")
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn list_spaces(app: axum::Router, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::get("/api/spaces")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_space(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/spaces/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn patch_space(
    app: axum::Router,
    cookie: &str,
    id: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::patch(format!("/api/spaces/{id}"))
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn reset_space(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::post(format!("/api/spaces/{id}/reset"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete_space(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/spaces/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn detect_space(app: axum::Router, cookie: &str, query: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/spaces/detect?{query}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn names(app: axum::Router, cookie: &str) -> Vec<String> {
    let listed = json_body(list_spaces(app, cookie).await).await;
    listed
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_then_list_round_trip(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-create@b.test", "Alice").await;

    let res = create_space(
        app.clone(),
        &cookie,
        serde_json::json!({ "name": "notes", "kind": "wiki" }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "create must return 201, matching Node's ctx.set.status = 201"
    );
    let created = json_body(res).await;
    assert_eq!(created["name"], "notes");
    assert_eq!(created["kind"], "wiki");
    assert_eq!(created["description"], serde_json::Value::Null);
    // Bare row: no nested `code` key on create.
    assert!(created.get("code").is_none());
    let id = created["id"].as_str().unwrap().to_string();

    let res = list_spaces(app.clone(), &cookie).await;
    assert_eq!(res.status(), StatusCode::OK);
    let listed = json_body(res).await;
    assert!(
        listed.is_array(),
        "GET /api/spaces must return a bare array, not an envelope"
    );
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["id"], id);
    assert!(
        listed[0].get("code").is_none(),
        "list rows are bare — no nested code key"
    );
    assert!(
        listed[0].get("remoteUrl").is_none(),
        "list never joins space_code_metadata, even flattened"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_defaults_kind_to_code(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-defaultkind@b.test", "Alice").await;

    let created = json_body(
        create_space(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "unspecified" }),
        )
        .await,
    )
    .await;
    assert_eq!(created["kind"], "code");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detail_nests_code_metadata_but_list_does_not(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-detail@b.test", "Alice").await;

    let created = json_body(
        create_space(
            app.clone(),
            &cookie,
            serde_json::json!({
                "name": "fubbik",
                "kind": "code",
                "remoteUrl": "git@github.com:acme/fubbik.git",
                "localPaths": ["/Users/alice/fubbik"]
            }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    let detail = json_body(get_space(app.clone(), &cookie, &id).await).await;
    assert!(
        detail.get("space").is_some() && detail.get("code").is_some(),
        "detail must be the nested {{space, code}} shape, got: {detail}"
    );
    assert_eq!(detail["space"]["id"], id);
    assert_eq!(detail["code"]["remoteUrl"], "github.com/acme/fubbik");
    assert_eq!(
        detail["code"]["localPaths"],
        serde_json::json!(["/Users/alice/fubbik"])
    );

    // The list endpoint, for the very same space, has no code key at all.
    let listed = json_body(list_spaces(app.clone(), &cookie).await).await;
    assert!(listed[0].get("code").is_none());
    assert!(listed[0].get("remoteUrl").is_none());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detail_of_a_non_code_space_has_null_code(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-wiki@b.test", "Alice").await;

    let created = json_body(
        create_space(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "notes", "kind": "wiki" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    let detail = json_body(get_space(app.clone(), &cookie, &id).await).await;
    assert_eq!(detail["code"], serde_json::Value::Null);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detect_match_returns_bare_space_without_a_code_key(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-detect@b.test", "Alice").await;

    let created = json_body(
        create_space(
            app.clone(),
            &cookie,
            serde_json::json!({
                "name": "fubbik",
                "kind": "code",
                "remoteUrl": "git@github.com:acme/fubbik.git",
                "localPaths": ["/Users/alice/fubbik"]
            }),
        )
        .await,
    )
    .await;

    let res = detect_space(
        app.clone(),
        &cookie,
        "remoteUrl=git%40github.com%3Aacme%2Ffubbik.git",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["id"], created["id"]);
    assert!(
        body.get("code").is_none(),
        "detect-match must be bare, no nested code key, unlike the detail endpoint"
    );
    assert!(body.get("remoteUrl").is_none());

    // Also findable by local path.
    let res = detect_space(app.clone(), &cookie, "localPath=%2FUsers%2Falice%2Ffubbik").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["id"], created["id"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detect_no_match_returns_a_genuinely_empty_body(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-detectnomatch@b.test", "Alice").await;

    let res = detect_space(
        app.clone(),
        &cookie,
        "remoteUrl=github.com%2Fnobody%2Fnothing",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "no-match is 200, not 404 — matching Node exactly"
    );
    assert!(
        res.headers().get("content-type").is_none(),
        "no-match must have no content-type header at all"
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert!(
        body.is_empty(),
        "no-match body must be genuinely empty, not \"null\" or \"{{}}\", got: {body:?}"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detect_with_no_query_params_returns_empty_body(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-detectnoparams@b.test", "Alice").await;

    let res = app
        .oneshot(
            Request::get("/api/spaces/detect")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert!(body.is_empty());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_with_duplicate_remote_url_is_400(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-dupe@b.test", "Alice").await;

    create_space(
        app.clone(),
        &cookie,
        serde_json::json!({ "name": "one", "kind": "code", "remoteUrl": "github.com/acme/fubbik" }),
    )
    .await;

    let res = create_space(
        app.clone(),
        &cookie,
        serde_json::json!({ "name": "two", "kind": "code", "remoteUrl": "git@github.com:acme/fubbik.git" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("A space with this remote URL already exists")
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_description_explicit_null_clears_it(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-clear@b.test", "Alice").await;

    let created = json_body(
        create_space(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "notes", "kind": "wiki", "description": "original" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // Omitted description must leave it untouched.
    let untouched = json_body(
        patch_space(
            app.clone(),
            &cookie,
            &id,
            serde_json::json!({ "name": "renamed" }),
        )
        .await,
    )
    .await;
    assert_eq!(untouched["description"], "original");

    // Explicit null must clear it.
    let cleared = json_body(
        patch_space(
            app.clone(),
            &cookie,
            &id,
            serde_json::json!({ "description": null }),
        )
        .await,
    )
    .await;
    assert_eq!(cleared["description"], serde_json::Value::Null);
    assert_eq!(
        cleared["name"], "renamed",
        "clearing description must not touch the name set by the previous PATCH"
    );
}

/// DELIBERATE DIVERGENCE FROM NODE (#3 in this slice — see
/// `service::update`'s doc comment for the full justification): Node
/// unconditionally overwrites `remoteUrl`/`localPaths` to `null`/`[]` on
/// ANY PATCH to a code-kind space that doesn't mention them, even one that
/// only touches `name`. This port does the opposite on purpose — omitted
/// fields must be left untouched, matching PATCH semantics everywhere else
/// in this codebase. Do not "fix" this back to Node's behavior.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patching_name_only_on_a_code_space_preserves_remote_url_and_local_paths(
    pool: sqlx::PgPool,
) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-clearcode@b.test", "Alice").await;

    let created = json_body(
        create_space(
            app.clone(),
            &cookie,
            serde_json::json!({
                "name": "fubbik",
                "kind": "code",
                "remoteUrl": "github.com/acme/fubbik",
                "localPaths": ["/Users/alice/fubbik"]
            }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    patch_space(
        app.clone(),
        &cookie,
        &id,
        serde_json::json!({ "name": "renamed" }),
    )
    .await;

    let detail = json_body(get_space(app.clone(), &cookie, &id).await).await;
    assert_eq!(detail["space"]["name"], "renamed");
    assert_eq!(
        detail["code"]["remoteUrl"], "github.com/acme/fubbik",
        "a PATCH not mentioning remoteUrl must leave it untouched"
    );
    assert_eq!(
        detail["code"]["localPaths"],
        serde_json::json!(["/Users/alice/fubbik"]),
        "a PATCH not mentioning localPaths must leave it untouched"
    );
}

/// The real-world path where Node's bug would actually bite: a user
/// renaming a code-kind space several times over (e.g. via a UI form that
/// only ever sends `{name}`) must never erode its git remote metadata.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn several_name_only_patches_do_not_erode_code_metadata(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-repeatedpatch@b.test", "Alice").await;

    let created = json_body(
        create_space(
            app.clone(),
            &cookie,
            serde_json::json!({
                "name": "fubbik",
                "kind": "code",
                "remoteUrl": "github.com/acme/fubbik",
                "localPaths": ["/Users/alice/fubbik"]
            }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    for name in ["renamed-once", "renamed-twice", "renamed-thrice"] {
        let res = patch_space(
            app.clone(),
            &cookie,
            &id,
            serde_json::json!({ "name": name }),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    let detail = json_body(get_space(app.clone(), &cookie, &id).await).await;
    assert_eq!(detail["space"]["name"], "renamed-thrice");
    assert_eq!(
        detail["code"]["remoteUrl"], "github.com/acme/fubbik",
        "remoteUrl must survive repeated name-only PATCHes"
    );
    assert_eq!(
        detail["code"]["localPaths"],
        serde_json::json!(["/Users/alice/fubbik"]),
        "localPaths must survive repeated name-only PATCHes"
    );
}

/// Omission is the only thing exempted from clearing — an explicit `null`
/// still clears `remoteUrl`, since Node's body schema (`t.Optional(t.Union([
/// t.String(...), t.Null()]))`) permits it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn explicit_null_still_clears_remote_url_on_a_code_space(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-explicitclear@b.test", "Alice").await;

    let created = json_body(
        create_space(
            app.clone(),
            &cookie,
            serde_json::json!({
                "name": "fubbik",
                "kind": "code",
                "remoteUrl": "github.com/acme/fubbik",
                "localPaths": ["/Users/alice/fubbik"]
            }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    patch_space(
        app.clone(),
        &cookie,
        &id,
        serde_json::json!({ "remoteUrl": null }),
    )
    .await;

    let detail = json_body(get_space(app.clone(), &cookie, &id).await).await;
    assert_eq!(detail["code"]["remoteUrl"], serde_json::Value::Null);
    assert_eq!(
        detail["code"]["localPaths"],
        serde_json::json!(["/Users/alice/fubbik"]),
        "localPaths must survive since this PATCH did not mention it"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_cannot_get_patch_or_delete_a_space(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-cross@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross@b.test", "Bob").await;

    let created = json_body(
        create_space(
            app.clone(),
            &alice_cookie,
            serde_json::json!({ "name": "notes", "kind": "wiki" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    let res = get_space(app.clone(), &bob_cookie, &id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = patch_space(
        app.clone(),
        &bob_cookie,
        &id,
        serde_json::json!({ "name": "hijacked" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = delete_space(app.clone(), &bob_cookie, &id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    assert_eq!(
        names(app.clone(), &alice_cookie).await,
        vec!["notes"],
        "Alice's space must survive Bob's rejected GET/PATCH/DELETE"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_cannot_reset_a_space(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-crossreset@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-crossreset@b.test", "Bob").await;

    let created = json_body(
        create_space(
            app.clone(),
            &alice_cookie,
            serde_json::json!({ "name": "notes", "kind": "wiki" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    let res = reset_space(app.clone(), &bob_cookie, &id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    assert_eq!(names(app.clone(), &alice_cookie).await, vec!["notes"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn reset_wipes_content_but_keeps_the_space_and_its_code_metadata(pool: sqlx::PgPool) {
    use fubbik_db::repo::{chunk, space, user};

    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-reset@b.test", "Alice").await;
    let alice_id = user::find_by_email(&pool, "alice-reset@b.test")
        .await
        .unwrap()
        .unwrap()
        .id;

    let created = json_body(
        create_space(
            app.clone(),
            &cookie,
            serde_json::json!({
                "name": "fubbik",
                "kind": "code",
                "remoteUrl": "github.com/acme/fubbik",
                "localPaths": ["/Users/alice/fubbik"]
            }),
        )
        .await,
    )
    .await;
    let space_id = created["id"].as_str().unwrap().to_string();

    let chunk_id = chunk::create(
        &pool,
        &alice_id,
        chunk::NewChunk {
            title: "Chunk".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id;
    space::set_chunk_spaces(&pool, &alice_id, &chunk_id, std::slice::from_ref(&space_id))
        .await
        .unwrap();

    let res = reset_space(app.clone(), &cookie, &space_id).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["chunksDeleted"], 1);
    assert_eq!(body["docsDeleted"], 0);
    assert_eq!(body["plansDeleted"], 0);
    assert_eq!(body["requirementsDeleted"], 0);

    assert!(
        chunk::find_by_id(&pool, &alice_id, &chunk_id)
            .await
            .unwrap()
            .is_none(),
        "the exclusive chunk must be hard-deleted"
    );

    // The space row and its code metadata survive.
    let detail = json_body(get_space(app.clone(), &cookie, &space_id).await).await;
    assert_eq!(detail["space"]["name"], "fubbik");
    assert_eq!(detail["code"]["remoteUrl"], "github.com/acme/fubbik");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_removes_the_space_row_after_wiping_content(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-delete@b.test", "Alice").await;

    let created = json_body(
        create_space(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "notes", "kind": "wiki" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    let res = delete_space(app.clone(), &cookie, &id).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Deleted");

    assert_eq!(names(app.clone(), &cookie).await, Vec::<String>::new());
    assert_eq!(
        get_space(app.clone(), &cookie, &id).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_request_is_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .oneshot(Request::get("/api/spaces").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
