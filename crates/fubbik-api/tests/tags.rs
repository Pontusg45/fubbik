//! HTTP-level tests for the `tags` domain.
//!
//! Node's captured contract (`tests/fixtures/node-contract/tags-list.json`,
//! `_mutating.md`) is the source of truth: `GET /api/tags` returns a bare
//! JSON array of *joined* rows (id, name, tagTypeId, tagTypeName,
//! tagTypeColor, tagTypeIcon, chunkCount); create/update return the *bare*
//! tag row instead (no joined fields); create is 201, update/delete/merge
//! are 200, delete's body is `{ "message": "Deleted" }`.
//!
//! `chunk_tag` is the first user-scoped many-to-many join in this codebase
//! — every mutating route here is exercised cross-user to prove a rejected
//! write leaves the victim's data untouched, not just that it 404s.

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
    }
}

/// Signs up a fresh user and returns the `name=value` session cookie pair
/// from the `set-cookie` response header, matching the pattern in
/// `tests/tag_types.rs::signup`.
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

async fn create_tag(
    app: axum::Router,
    cookie: &str,
    name: &str,
    tag_type_id: Option<&str>,
) -> axum::response::Response {
    let mut body = serde_json::json!({ "name": name });
    if let Some(tag_type_id) = tag_type_id {
        body["tagTypeId"] = serde_json::Value::String(tag_type_id.to_string());
    }
    app.oneshot(
        Request::post("/api/tags")
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn list_tags(app: axum::Router, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::get("/api/tags")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn patch_tag(
    app: axum::Router,
    cookie: &str,
    id: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::patch(format!("/api/tags/{id}"))
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete_tag(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/tags/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn merge_tags(
    app: axum::Router,
    cookie: &str,
    source_id: &str,
    target_id: &str,
) -> axum::response::Response {
    let body = serde_json::json!({ "sourceId": source_id, "targetId": target_id }).to_string();
    app.oneshot(
        Request::post("/api/tags/merge")
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn names(app: axum::Router, cookie: &str) -> Vec<String> {
    let listed = json_body(list_tags(app, cookie).await).await;
    listed
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_then_list_round_trip(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-create@b.test", "Alice").await;

    let res = create_tag(app.clone(), &cookie, "rust", None).await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "create must return 201, matching Node's ctx.set.status = 201"
    );
    let created = json_body(res).await;
    assert_eq!(created["name"], "rust");
    assert_eq!(created["tagTypeId"], serde_json::Value::Null);
    assert_eq!(created["reviewStatus"], "approved");
    assert_eq!(created["origin"], "human");
    // Bare row: no joined `tagTypeName`/`chunkCount` fields on create.
    assert!(created.get("tagTypeName").is_none());
    assert!(created.get("chunkCount").is_none());
    let id = created["id"].as_str().unwrap().to_string();

    let res = list_tags(app.clone(), &cookie).await;
    assert_eq!(res.status(), StatusCode::OK);
    let listed = json_body(res).await;
    assert!(
        listed.is_array(),
        "GET /api/tags must return a bare array, not the {{chunks,...}} envelope"
    );
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["id"], id);
    assert_eq!(listed[0]["name"], "rust");
    assert_eq!(listed[0]["tagTypeName"], serde_json::Value::Null);
    assert_eq!(listed[0]["chunkCount"], 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_joins_tag_type_fields(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-jointype@b.test", "Alice").await;

    let user_id: String =
        sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = 'alice-jointype@b.test'"#)
            .fetch_one(&pool)
            .await
            .unwrap();
    let tag_type_id = fubbik_db::new_id();
    sqlx::query!(
        "INSERT INTO tag_type (id, name, color, user_id) VALUES ($1, 'topic', '#ff0000', $2)",
        tag_type_id,
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let created =
        json_body(create_tag(app.clone(), &cookie, "rust", Some(&tag_type_id)).await).await;
    assert_eq!(created["tagTypeId"], tag_type_id);

    let listed = json_body(list_tags(app.clone(), &cookie).await).await;
    assert_eq!(listed[0]["tagTypeId"], tag_type_id);
    assert_eq!(listed[0]["tagTypeName"], "topic");
    assert_eq!(listed[0]["tagTypeColor"], "#ff0000");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_patch_is_404_and_leaves_row_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-patch@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-patch@b.test", "Bob").await;

    let created = json_body(create_tag(app.clone(), &alice_cookie, "rust", None).await).await;
    let id = created["id"].as_str().unwrap().to_string();

    let res = patch_tag(
        app.clone(),
        &bob_cookie,
        &id,
        serde_json::json!({ "name": "hijacked" }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to update this tag"
    );

    assert_eq!(
        names(app.clone(), &alice_cookie).await,
        vec!["rust"],
        "Alice's tag must survive Bob's rejected PATCH"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_delete_is_404_and_leaves_row_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-delete@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-delete@b.test", "Bob").await;

    let created = json_body(create_tag(app.clone(), &alice_cookie, "rust", None).await).await;
    let id = created["id"].as_str().unwrap().to_string();

    let res = delete_tag(app.clone(), &bob_cookie, &id).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to delete this tag"
    );

    assert_eq!(
        names(app.clone(), &alice_cookie).await,
        vec!["rust"],
        "Alice's tag must survive Bob's rejected DELETE"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_cascades_chunk_tag(pool: sqlx::PgPool) {
    use fubbik_db::repo::{chunk, tag};

    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-cascade@b.test", "Alice").await;
    let user_id: String =
        sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = 'alice-cascade@b.test'"#)
            .fetch_one(&pool)
            .await
            .unwrap();

    let created_tag = tag::create(&pool, &user_id, "rust", None).await.unwrap();
    let chunk_id = chunk::create(
        &pool,
        &user_id,
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
    tag::set_chunk_tags(
        &pool,
        &user_id,
        &chunk_id,
        std::slice::from_ref(&created_tag.id),
    )
    .await
    .unwrap();

    let res = delete_tag(app.clone(), &cookie, &created_tag.id).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Deleted");

    let remaining: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM chunk_tag WHERE tag_id = $1"#,
        created_tag.id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        remaining, 0,
        "chunk_tag rows must cascade-delete via the FK, not be left dangling"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_name_conflict_is_400_and_leaves_row_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-conflict@b.test", "Alice").await;

    json_body(create_tag(app.clone(), &cookie, "rust", None).await).await;
    let second = json_body(create_tag(app.clone(), &cookie, "postgres", None).await).await;
    let second_id = second["id"].as_str().unwrap().to_string();

    let res = patch_tag(
        app.clone(),
        &cookie,
        &second_id,
        serde_json::json!({ "name": "rust" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    // `AppError::Validation`'s `Display` prepends "validation failed: " to
    // every message in this framework (see `fubbik_core::error::AppError`)
    // — an established, crate-wide convention, not something specific to
    // this route — so this checks the Node-sourced text is present rather
    // than asserting byte-for-byte equality against Node's raw message.
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("Tag \"rust\" already exists")
    );

    let mut found = names(app.clone(), &cookie).await;
    found.sort();
    assert_eq!(
        found,
        vec!["postgres".to_string(), "rust".to_string()],
        "the rejected rename must not have mutated either tag's name"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_tag_type_id_explicit_null_clears_it(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-clear@b.test", "Alice").await;
    let user_id: String =
        sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = 'alice-clear@b.test'"#)
            .fetch_one(&pool)
            .await
            .unwrap();
    let tag_type_id = fubbik_db::new_id();
    sqlx::query!(
        "INSERT INTO tag_type (id, name, user_id) VALUES ($1, 'topic', $2)",
        tag_type_id,
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let created =
        json_body(create_tag(app.clone(), &cookie, "rust", Some(&tag_type_id)).await).await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["tagTypeId"], tag_type_id);

    // Omitted `tagTypeId` must leave it untouched.
    let untouched = json_body(
        patch_tag(
            app.clone(),
            &cookie,
            &id,
            serde_json::json!({ "name": "rust2" }),
        )
        .await,
    )
    .await;
    assert_eq!(untouched["tagTypeId"], tag_type_id);

    // Explicit `null` must clear it.
    let cleared = json_body(
        patch_tag(
            app.clone(),
            &cookie,
            &id,
            serde_json::json!({ "tagTypeId": null }),
        )
        .await,
    )
    .await;
    assert_eq!(cleared["tagTypeId"], serde_json::Value::Null);
    assert_eq!(
        cleared["name"], "rust2",
        "clearing tagTypeId must not touch the name set by the previous PATCH"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_review_status_sets_reviewed_by_and_at(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-review@b.test", "Alice").await;

    let created = json_body(create_tag(app.clone(), &cookie, "rust", None).await).await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["reviewedBy"], serde_json::Value::Null);
    assert_eq!(created["reviewedAt"], serde_json::Value::Null);

    let updated = json_body(
        patch_tag(
            app.clone(),
            &cookie,
            &id,
            serde_json::json!({ "reviewStatus": "reviewed" }),
        )
        .await,
    )
    .await;
    assert_eq!(updated["reviewStatus"], "reviewed");
    assert!(updated["reviewedBy"].is_string());
    assert!(updated["reviewedAt"].is_string());
}

/// `reviewStatus` is a proper enum (`tags::dto::ReviewStatus`), matching
/// Node's `t.Union([t.Literal("draft"), t.Literal("reviewed"),
/// t.Literal("approved")])` — every one of those three literals must be
/// accepted.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_review_status_accepts_every_valid_value(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-review-valid@b.test", "Alice").await;

    let created = json_body(create_tag(app.clone(), &cookie, "rust", None).await).await;
    let id = created["id"].as_str().unwrap().to_string();

    for value in ["draft", "reviewed", "approved"] {
        let res = patch_tag(
            app.clone(),
            &cookie,
            &id,
            serde_json::json!({ "reviewStatus": value }),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "{value} must be accepted as a valid reviewStatus"
        );
        let body = json_body(res).await;
        assert_eq!(body["reviewStatus"], value);
    }
}

/// Verified live against the pre-fix Rust server: `{"reviewStatus":
/// "totally-bogus"}` returned 200, persisted the garbage value, and set
/// `reviewedBy`/`reviewedAt` alongside it — `review_status` was a plain
/// `Option<String>`, and there is no database check constraint
/// backstopping the column. This proves the enum rejects it before it
/// reaches the database, by reading `review_status`/`reviewed_by`/
/// `reviewed_at`/`name` back from the row directly rather than trusting
/// just the response status code.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_review_status_invalid_value_is_rejected_and_leaves_row_unchanged(
    pool: sqlx::PgPool,
) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-review-invalid@b.test", "Alice").await;

    let created = json_body(create_tag(app.clone(), &cookie, "rust", None).await).await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["reviewStatus"], "approved");

    let res = patch_tag(
        app.clone(),
        &cookie,
        &id,
        serde_json::json!({ "reviewStatus": "totally-bogus" }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "an invalid reviewStatus literal must be rejected at deserialisation"
    );

    let row = sqlx::query!(
        r#"SELECT name, review_status, reviewed_by, reviewed_at FROM tag WHERE id = $1"#,
        id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.name, "rust", "rejected PATCH must not touch name");
    assert_eq!(
        row.review_status, "approved",
        "rejected PATCH must not touch review_status"
    );
    assert_eq!(
        row.reviewed_by, None,
        "rejected PATCH must not set reviewed_by"
    );
    assert_eq!(
        row.reviewed_at, None,
        "rejected PATCH must not set reviewed_at"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn merge_moves_chunk_count_and_deletes_source(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-merge@b.test", "Alice").await;

    let source = json_body(create_tag(app.clone(), &cookie, "source", None).await).await;
    let target = json_body(create_tag(app.clone(), &cookie, "target", None).await).await;

    let res = merge_tags(
        app.clone(),
        &cookie,
        source["id"].as_str().unwrap(),
        target["id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["targetId"], target["id"]);
    assert_eq!(body["chunkCount"], 0);

    assert_eq!(names(app.clone(), &cookie).await, vec!["target"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn merge_into_self_is_400(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-selfmerge@b.test", "Alice").await;

    let tag = json_body(create_tag(app.clone(), &cookie, "solo", None).await).await;
    let id = tag["id"].as_str().unwrap().to_string();

    let res = merge_tags(app.clone(), &cookie, &id, &id).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    // See the comment in `update_name_conflict_is_400_and_leaves_row_unchanged`
    // on why this is a substring check, not an exact match.
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("Cannot merge a tag into itself")
    );

    assert_eq!(names(app.clone(), &cookie).await, vec!["solo"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn merge_unknown_source_id_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-mergeunknown@b.test", "Alice").await;

    let target = json_body(create_tag(app.clone(), &cookie, "target", None).await).await;

    let res = merge_tags(
        app.clone(),
        &cookie,
        "does-not-exist",
        target["id"].as_str().unwrap(),
    )
    .await;
    // APPROVED DIVERGENCE: Node throws an untagged Error here (-> 500);
    // this returns AppError::NotFound (-> 404) instead.
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    assert_eq!(names(app.clone(), &cookie).await, vec!["target"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn merge_of_another_users_tag_is_404_and_leaves_both_users_data_unchanged(
    pool: sqlx::PgPool,
) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-crossmerge@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-crossmerge@b.test", "Bob").await;

    let alices_tag = json_body(create_tag(app.clone(), &alice_cookie, "alices", None).await).await;
    let bobs_tag = json_body(create_tag(app.clone(), &bob_cookie, "bobs", None).await).await;

    // Alice tries to merge Bob's tag into her own — must be rejected.
    let res = merge_tags(
        app.clone(),
        &alice_cookie,
        bobs_tag["id"].as_str().unwrap(),
        alices_tag["id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Neither user's tag list was mutated by the rejected merge.
    assert_eq!(names(app.clone(), &alice_cookie).await, vec!["alices"]);
    assert_eq!(names(app.clone(), &bob_cookie).await, vec!["bobs"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_request_is_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .oneshot(Request::get("/api/tags").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
