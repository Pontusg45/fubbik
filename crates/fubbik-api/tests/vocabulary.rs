//! HTTP-level tests for the `vocabulary` domain
//! (`packages/api/src/vocabulary/routes.ts`). Repo-level CRUD and the
//! `EXISTS`-guard proofs live in `fubbik-db/tests/vocabulary.rs`; this file
//! covers the seven routes: status codes, response shapes, the
//! `spaceId`-absent-returns-`[]` quirk, the auto-seed-on-first-entry
//! behaviour end to end, and — the highest-severity cases here — that a
//! foreign space/entry 404s through the full HTTP stack *and* leaves the
//! victim's data unchanged (proving the service-layer
//! `verify_space_ownership` guard, on top of the repo-layer `EXISTS` guard
//! already proven in `fubbik-db/tests/vocabulary.rs`).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
        better_auth_secret: "test-secret".into(),
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
        rate_limiter: Default::default(),
        background: Default::default(),
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

async fn seed_chunk(
    pool: &sqlx::PgPool,
    user_id: &str,
    space_id: &str,
    title: &str,
    content: &str,
) {
    let chunk = fubbik_db::repo::chunk::create(
        pool,
        user_id,
        fubbik_db::repo::chunk::NewChunk {
            title: title.into(),
            content: content.into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    fubbik_db::repo::space::set_chunk_spaces(pool, user_id, &chunk.id, &[space_id.to_string()])
        .await
        .unwrap();
}

async fn get_vocabulary(
    app: axum::Router,
    cookie: &str,
    space_id: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/vocabulary?spaceId={space_id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn create_entry(
    app: axum::Router,
    cookie: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/vocabulary")
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn update_entry(
    app: axum::Router,
    cookie: &str,
    id: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::patch(format!("/api/vocabulary/{id}"))
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete_entry(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/vocabulary/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_returns_empty_array_when_space_id_is_absent(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-noquery@b.test", "Alice").await;

    // When
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/vocabulary")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await, serde_json::json!([]));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_404s_for_a_space_the_caller_does_not_own(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let owner_cookie = signup(app.clone(), "owner-list@b.test", "Owner").await;
    let attacker_cookie = signup(app.clone(), "attacker-list@b.test", "Attacker").await;
    let owner_id = user_id_for_email(&pool, "owner-list@b.test").await;
    let sid = seed_space(&pool, &owner_id, "Owner space").await;

    create_entry(
        app.clone(),
        &owner_cookie,
        serde_json::json!({ "word": "click", "category": "action", "spaceId": sid }),
    )
    .await;

    // When
    let res = get_vocabulary(app.clone(), &attacker_cookie, &sid).await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // The owner's own view must be unaffected by the attacker's attempt.
    let owners_res = get_vocabulary(app.clone(), &owner_cookie, &sid).await;
    assert_eq!(owners_res.status(), StatusCode::OK);
    let body = json_body(owners_res).await;
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|e| e["word"] == "click")
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_entry_returns_201_lowercases_word_and_auto_seeds_modifiers_once(
    pool: sqlx::PgPool,
) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-create@b.test", "Alice").await;
    let uid = user_id_for_email(&pool, "alice-create@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;

    // When
    let res = create_entry(
        app.clone(),
        &cookie,
        serde_json::json!({ "word": "ClICk", "category": "action", "expects": ["target"], "spaceId": sid }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["word"], "click");
    assert_eq!(body["category"], "action");
    assert_eq!(body["expects"], serde_json::json!(["target"]));
    assert_eq!(body["spaceId"], sid);
    assert!(body["definition"].is_null());

    // First entry in the space auto-seeds the 16 standard modifiers.
    let list_res = get_vocabulary(app.clone(), &cookie, &sid).await;
    let list_body = json_body(list_res).await;
    let entries = list_body.as_array().unwrap();
    assert_eq!(entries.len(), 17, "1 created entry + 16 seeded modifiers");
    let modifier_count = entries
        .iter()
        .filter(|e| e["category"] == "modifier")
        .count();
    assert_eq!(modifier_count, 16);

    // A second entry must not reseed.
    create_entry(
        app.clone(),
        &cookie,
        serde_json::json!({ "word": "button", "category": "target", "spaceId": sid }),
    )
    .await;
    let list_res2 = get_vocabulary(app.clone(), &cookie, &sid).await;
    let entries2 = json_body(list_res2).await;
    assert_eq!(entries2.as_array().unwrap().len(), 18);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_entry_404s_for_a_space_the_caller_does_not_own(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let owner_cookie = signup(app.clone(), "owner-ce@b.test", "Owner").await;
    let attacker_cookie = signup(app.clone(), "attacker-ce@b.test", "Attacker").await;
    let owner_id = user_id_for_email(&pool, "owner-ce@b.test").await;
    let sid = seed_space(&pool, &owner_id, "Owner space").await;

    // When
    let res = create_entry(
        app.clone(),
        &attacker_cookie,
        serde_json::json!({ "word": "click", "category": "action", "spaceId": sid }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let owners_view = json_body(get_vocabulary(app.clone(), &owner_cookie, &sid).await).await;
    assert_eq!(
        owners_view.as_array().unwrap().len(),
        0,
        "attacker's create attempt must not have landed"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn bulk_create_returns_201_and_skips_conflicting_entries(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-bulk@b.test", "Alice").await;
    let uid = user_id_for_email(&pool, "alice-bulk@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;

    create_entry(
        app.clone(),
        &cookie,
        serde_json::json!({ "word": "click", "category": "action", "spaceId": sid }),
    )
    .await;

    // When
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/vocabulary/bulk")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "entries": [
                            { "word": "click", "category": "action" },
                            { "word": "Button", "category": "target" }
                        ],
                        "spaceId": sid
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    let created = body.as_array().unwrap();
    assert_eq!(
        created.len(),
        1,
        "the conflicting \"click\"/action row must be skipped"
    );
    assert_eq!(created[0]["word"], "button");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn parse_matches_the_spaces_vocabulary(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-parse@b.test", "Alice").await;
    let uid = user_id_for_email(&pool, "alice-parse@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;

    create_entry(
        app.clone(),
        &cookie,
        serde_json::json!({ "word": "click", "category": "action", "expects": ["target"], "spaceId": sid }),
    )
    .await;
    create_entry(
        app.clone(),
        &cookie,
        serde_json::json!({ "word": "button", "category": "target", "spaceId": sid }),
    )
    .await;

    // When
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/vocabulary/parse")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "text": "click the button", "spaceId": sid }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["warnings"], serde_json::json!([]));
    let tokens = body["tokens"].as_array().unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0]["text"], "click");
    assert_eq!(tokens[0]["category"], "action");
    assert_eq!(tokens[1]["text"], "the");
    assert_eq!(tokens[1]["category"], "modifier");
    assert_eq!(tokens[2]["text"], "button");
    assert_eq!(tokens[2]["category"], "target");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn suggest_404s_for_a_foreign_space_and_200s_with_an_array_otherwise(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let owner_cookie = signup(app.clone(), "owner-sug@b.test", "Owner").await;
    let attacker_cookie = signup(app.clone(), "attacker-sug@b.test", "Attacker").await;
    let owner_id = user_id_for_email(&pool, "owner-sug@b.test").await;
    let sid = seed_space(&pool, &owner_id, "Owner space").await;

    // When
    let foreign_res = app
        .clone()
        .oneshot(
            Request::post("/api/vocabulary/suggest")
                .header("cookie", &attacker_cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "spaceId": sid }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(foreign_res.status(), StatusCode::NOT_FOUND);

    // No Ollama server is running in this test environment, so this must
    // degrade to `200 []`, never a 500 — see
    // `vocabulary::suggest::suggest_vocabulary`'s doc comment.
    let owned_res = app
        .clone()
        .oneshot(
            Request::post("/api/vocabulary/suggest")
                .header("cookie", &owner_cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "spaceId": sid }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(owned_res.status(), StatusCode::OK);
    assert_eq!(json_body(owned_res).await, serde_json::json!([]));
}

/// The success path of `suggest_vocabulary` — parsing entries out of a
/// prose-wrapped model response, and dropping an entry with an unknown
/// category — has never been executed by any test in this repository
/// before this one: reaching it previously required a real Ollama server
/// running locally. `state()` now injects a client, so a `wiremock` server
/// standing in for Ollama exercises it here for the first time.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn suggest_returns_entries_from_the_model(pool: sqlx::PgPool) {
    // Given
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/generate"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "Here you go: [{\"word\":\"user\",\"category\":\"actor\",\"expects\":[\"action\"]},{\"word\":\"x\",\"category\":\"bogus\"}]"
            })),
        )
        .mount(&server)
        .await;

    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "vocab-suggest@b.test", "Suggester").await;
    let user_id = user_id_for_email(&pool, "vocab-suggest@b.test").await;
    let sid = seed_space(&pool, &user_id, "Suggest space").await;
    seed_chunk(&pool, &user_id, &sid, "How auth works", "The user logs in.").await;

    // When
    let res = app
        .oneshot(
            Request::post("/api/vocabulary/suggest")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "spaceId": sid }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::OK);

    let body = json_body(res).await;
    // The prose wrapper is stripped, the valid entry survives, and the
    // entry with an unknown category is dropped by the validation loop.
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["word"], "user");
    assert_eq!(body[0]["category"], "actor");
    assert_eq!(body[0]["expects"][0], "action");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_returns_200_and_404s_with_unchanged_data_for_another_users_entry(
    pool: sqlx::PgPool,
) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let owner_cookie = signup(app.clone(), "owner-upd@b.test", "Owner").await;
    let attacker_cookie = signup(app.clone(), "attacker-upd@b.test", "Attacker").await;
    let owner_id = user_id_for_email(&pool, "owner-upd@b.test").await;
    let sid = seed_space(&pool, &owner_id, "Owner space").await;

    let created = json_body(
        create_entry(
            app.clone(),
            &owner_cookie,
            serde_json::json!({ "word": "click", "category": "action", "spaceId": sid }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    // When
    let ok_res = update_entry(
        app.clone(),
        &owner_cookie,
        id,
        serde_json::json!({ "word": "tap" }),
    )
    .await;
    // Then
    assert_eq!(ok_res.status(), StatusCode::OK);
    assert_eq!(json_body(ok_res).await["word"], "tap");

    let foreign_res = update_entry(
        app.clone(),
        &attacker_cookie,
        id,
        serde_json::json!({ "word": "hacked" }),
    )
    .await;
    assert_eq!(foreign_res.status(), StatusCode::NOT_FOUND);

    let unchanged = fubbik_db::repo::vocabulary::get_by_id(&pool, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        unchanged.word, "tap",
        "attacker's update must not have landed"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_returns_200_and_404s_leaving_another_users_entry_intact(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let owner_cookie = signup(app.clone(), "owner-del@b.test", "Owner").await;
    let attacker_cookie = signup(app.clone(), "attacker-del@b.test", "Attacker").await;
    let owner_id = user_id_for_email(&pool, "owner-del@b.test").await;
    let sid = seed_space(&pool, &owner_id, "Owner space").await;

    let created = json_body(
        create_entry(
            app.clone(),
            &owner_cookie,
            serde_json::json!({ "word": "click", "category": "action", "spaceId": sid }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    // When
    let foreign_res = delete_entry(app.clone(), &attacker_cookie, id).await;
    // Then
    assert_eq!(foreign_res.status(), StatusCode::NOT_FOUND);
    assert!(
        fubbik_db::repo::vocabulary::get_by_id(&pool, id)
            .await
            .unwrap()
            .is_some(),
        "victim's entry must still exist after attacker's failed delete"
    );

    let owner_res = delete_entry(app.clone(), &owner_cookie, id).await;
    assert_eq!(owner_res.status(), StatusCode::OK);
    assert_eq!(
        json_body(owner_res).await,
        serde_json::json!({ "message": "Deleted" })
    );
    assert!(
        fubbik_db::repo::vocabulary::get_by_id(&pool, id)
            .await
            .unwrap()
            .is_none()
    );
}
