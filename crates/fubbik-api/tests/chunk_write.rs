//! HTTP-level tests for the chunk **write** surface — `POST /api/chunks`
//! and `PATCH /api/chunks/{id}`.
//!
//! Both bodies used to accept a fraction of Node's field set, and serde
//! drops unknown fields rather than rejecting them, so every missing field
//! was a **silent** no-op: the request returned success and the data never
//! landed. That is the same failure mode that made `requirements/stats`
//! ignore `spaceId`. These tests exist to make each field's arrival
//! observable.

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
    serde_json::from_slice(&body).unwrap()
}

async fn send(
    app: axum::Router,
    cookie: &str,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get(app: axum::Router, cookie: &str, path: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(path.to_string())
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

/// The detail envelope for a chunk, so a test can assert on the row *and*
/// its join tables in one read.
async fn detail(app: axum::Router, cookie: &str, id: &str) -> serde_json::Value {
    let res = get(app, cookie, &format!("/api/chunks/{id}")).await;
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await
}

async fn create(
    app: axum::Router,
    cookie: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    send(app, cookie, "POST", "/api/chunks", body).await
}

async fn reject_chunk_tag_inserts(pool: &sqlx::PgPool) {
    sqlx::query(
        r#"CREATE FUNCTION reject_chunk_tag_insert() RETURNS trigger
           LANGUAGE plpgsql AS $$
           BEGIN
             RAISE EXCEPTION 'forced chunk-tag failure';
           END
           $$"#,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"CREATE TRIGGER reject_chunk_tag_insert
           BEFORE INSERT ON chunk_tag
           FOR EACH ROW EXECUTE FUNCTION reject_chunk_tag_insert()"#,
    )
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_rolls_back_chunk_and_new_tags_when_linking_fails(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "rollback-create@b.test", "Alice").await;
    reject_chunk_tag_inserts(&pool).await;

    let response = create(
        app,
        &cookie,
        serde_json::json!({ "title": "must roll back", "tags": ["also-rolls-back"] }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let chunk_count: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk WHERE title = $1")
        .bind("must roll back")
        .fetch_one(&pool)
        .await
        .unwrap();
    let tag_count: i64 = sqlx::query_scalar("SELECT count(*) FROM tag WHERE name = $1")
        .bind("also-rolls-back")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(chunk_count, 0, "the aggregate root must roll back");
    assert_eq!(tag_count, 0, "new aggregate metadata must roll back");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_rolls_back_row_history_and_tags_when_linking_fails(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "rollback-update@b.test", "Alice").await;
    let id = json_body(
        create(
            app.clone(),
            &cookie,
            serde_json::json!({ "title": "original", "tags": ["original-tag"] }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    reject_chunk_tag_inserts(&pool).await;

    let response = send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/chunks/{id}"),
        serde_json::json!({ "title": "changed", "tags": ["new-tag"] }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = detail(app, &cookie, &id).await;
    assert_eq!(body["chunk"]["title"], "original");
    assert_eq!(body["tags"][0]["name"], "original-tag");
    let history_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM chunk_version WHERE chunk_id = $1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(history_count, 0, "the pre-update snapshot must roll back");
}

/// Every field `POST /api/chunks` documents actually lands.
///
/// Enumerated rather than spot-checked: each of these was silently dropped
/// before, and checking two of them would have left the rest live.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_persists_every_documented_field(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;

    let space = json_body(
        send(
            app.clone(),
            &cookie,
            "POST",
            "/api/spaces",
            serde_json::json!({ "name": "alpha", "kind": "code" }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = create(
        app.clone(),
        &cookie,
        serde_json::json!({
            "title": "  Trimmed title  ",
            "content": "body text",
            "type": "reference",
            "tags": ["alpha", "beta"],
            "spaceIds": [space],
            "rationale": "because",
            "alternatives": ["do nothing", "wait"],
            "consequences": "some fallout"
        }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "Node answers 201 on chunk create, not 200"
    );
    let id = json_body(res).await["id"].as_str().unwrap().to_string();

    let body = detail(app, &cookie, &id).await;
    let chunk = &body["chunk"];
    assert_eq!(chunk["title"], "Trimmed title", "the title is trimmed");
    assert_eq!(chunk["content"], "body text");
    assert_eq!(chunk["type"], "reference");
    assert_eq!(chunk["rationale"], "because");
    assert_eq!(
        chunk["alternatives"],
        serde_json::json!(["do nothing", "wait"])
    );
    assert_eq!(chunk["consequences"], "some fallout");

    let mut tags: Vec<&str> = body["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    tags.sort_unstable();
    assert_eq!(
        tags,
        ["alpha", "beta"],
        "tags arrive as names and are created"
    );

    let spaces: Vec<&str> = body["spaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(spaces, [space.as_str()]);
}

/// `origin` decides `reviewStatus`: an AI-authored chunk lands as a draft
/// needing review, a human-authored one lands approved. Both directions
/// asserted, so the test cannot pass on a hardcoded default.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_derives_review_status_from_origin(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;

    for (origin, expected) in [
        (Some("ai"), "draft"),
        (Some("human"), "approved"),
        (None, "approved"),
    ] {
        let mut body = serde_json::json!({ "title": "T" });
        if let Some(o) = origin {
            body["origin"] = serde_json::json!(o);
        }
        let created = json_body(create(app.clone(), &cookie, body).await).await;
        assert_eq!(
            created["reviewStatus"], expected,
            "origin {origin:?} must produce reviewStatus {expected}"
        );
        assert_eq!(created["origin"], origin.unwrap_or("human"));
    }
}

/// A `documentId` the caller does not own is a 400, not a silently
/// unlinked chunk. A blank one is treated as absent, matching Node.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_rejects_an_unknown_document_id_but_ignores_a_blank_one(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;

    let res = create(
        app.clone(),
        &cookie,
        serde_json::json!({ "title": "T", "documentId": "no-such-document" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let res = create(
        app.clone(),
        &cookie,
        serde_json::json!({ "title": "T", "documentId": "   " }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "a whitespace-only documentId is absent, not invalid"
    );
    assert_eq!(json_body(res).await["documentId"], serde_json::Value::Null);
}

/// Every field `PATCH /api/chunks/{id}` documents actually lands.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patch_persists_every_documented_field(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = json_body(create(app.clone(), &cookie, serde_json::json!({ "title": "T" })).await)
        .await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/chunks/{id}"),
        serde_json::json!({
            "title": "New title",
            "content": "new content",
            "type": "schema",
            "tags": ["gamma"],
            "summary": "a summary",
            "aliases": ["alias-one"],
            "notAbout": ["not this"],
            "scope": { "env": "dev" },
            "rationale": "why",
            "alternatives": ["alt"],
            "consequences": "fallout",
            "reviewStatus": "reviewed",
            "isEntryPoint": true
        }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = detail(app, &cookie, &id).await;
    let chunk = &body["chunk"];
    assert_eq!(chunk["title"], "New title");
    assert_eq!(chunk["content"], "new content");
    assert_eq!(chunk["type"], "schema");
    assert_eq!(chunk["summary"], "a summary");
    assert_eq!(chunk["aliases"], serde_json::json!(["alias-one"]));
    assert_eq!(chunk["notAbout"], serde_json::json!(["not this"]));
    assert_eq!(chunk["scope"], serde_json::json!({ "env": "dev" }));
    assert_eq!(chunk["rationale"], "why");
    assert_eq!(chunk["alternatives"], serde_json::json!(["alt"]));
    assert_eq!(chunk["consequences"], "fallout");
    assert_eq!(chunk["reviewStatus"], "reviewed");
    assert_eq!(chunk["isEntryPoint"], true);

    let tags: Vec<&str> = body["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(tags, ["gamma"]);

    // Setting `reviewStatus` stamps who asserted it and when — the client
    // cannot supply either.
    // The stamp must be the *caller*, not merely non-empty — `userId` is on
    // the same row, so this compares against a known value rather than
    // asserting "something was written".
    assert_eq!(
        chunk["reviewedBy"], chunk["userId"],
        "reviewedBy must be stamped with the caller when reviewStatus is set"
    );
    assert!(
        chunk["reviewedAt"].is_string(),
        "reviewedAt must be stamped alongside reviewedBy"
    );
}

/// `summary` is the one tri-state field: absent leaves it, `null` clears
/// it, a string sets it. All three transitions asserted in sequence —
/// checking only "set" would pass against a two-state implementation.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patch_summary_is_tri_state(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = json_body(create(app.clone(), &cookie, serde_json::json!({ "title": "T" })).await)
        .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let path = format!("/api/chunks/{id}");

    // set
    send(
        app.clone(),
        &cookie,
        "PATCH",
        &path,
        serde_json::json!({ "summary": "first" }),
    )
    .await;
    assert_eq!(
        detail(app.clone(), &cookie, &id).await["chunk"]["summary"],
        "first"
    );

    // absent — must leave the existing value alone
    send(
        app.clone(),
        &cookie,
        "PATCH",
        &path,
        serde_json::json!({ "title": "unrelated edit" }),
    )
    .await;
    assert_eq!(
        detail(app.clone(), &cookie, &id).await["chunk"]["summary"],
        "first",
        "omitting `summary` must not clear it"
    );

    // explicit null — must clear
    send(
        app.clone(),
        &cookie,
        "PATCH",
        &path,
        serde_json::json!({ "summary": null }),
    )
    .await;
    assert_eq!(
        detail(app, &cookie, &id).await["chunk"]["summary"],
        serde_json::Value::Null,
        "an explicit null must clear the summary"
    );
}

/// `tags: []` clears, `tags` absent leaves alone. The distinction matters:
/// a naive implementation that treats an empty vec as "nothing to do"
/// makes "remove every tag" unreachable from the UI.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patch_distinguishes_empty_tags_from_absent_tags(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = json_body(
        create(
            app.clone(),
            &cookie,
            serde_json::json!({ "title": "T", "tags": ["keep"] }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let path = format!("/api/chunks/{id}");

    send(
        app.clone(),
        &cookie,
        "PATCH",
        &path,
        serde_json::json!({ "title": "unrelated" }),
    )
    .await;
    assert_eq!(
        detail(app.clone(), &cookie, &id).await["tags"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "omitting `tags` must leave them alone"
    );

    send(
        app.clone(),
        &cookie,
        "PATCH",
        &path,
        serde_json::json!({ "tags": [] }),
    )
    .await;
    assert_eq!(
        detail(app, &cookie, &id).await["tags"]
            .as_array()
            .unwrap()
            .len(),
        0,
        "`tags: []` must clear them"
    );
}

/// A tag name already owned by the caller is reused, not duplicated —
/// otherwise `tag_user_name_idx` would reject the second chunk outright.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn tags_are_reused_across_chunks_rather_than_duplicated(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;

    for title in ["first", "second"] {
        let res = create(
            app.clone(),
            &cookie,
            serde_json::json!({ "title": title, "tags": ["shared"] }),
        )
        .await;
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    let tags = json_body(get(app, &cookie, "/api/tags").await).await;
    let shared: Vec<_> = tags
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["name"] == "shared")
        .collect();
    assert_eq!(shared.len(), 1, "the tag must be reused, not duplicated");
    assert_eq!(shared[0]["chunkCount"], 2);
}

/// The version snapshot taken before an update carries `alternatives`,
/// `scope` and `updateTag` — three columns `chunk_version` has always had
/// and this port never wrote, so `GET /chunks/{id}/history` served null for
/// them regardless of the chunk's contents.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn history_records_alternatives_scope_and_update_tag(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = json_body(
        create(
            app.clone(),
            &cookie,
            serde_json::json!({
                "title": "Original",
                "alternatives": ["the road not taken"]
            }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // `scope` is dropped on create (Node parity — see
    // `create_accepts_scope_and_drops_it_like_node`), so it is set through
    // PATCH, which is where it lands. This first PATCH is itself
    // snapshotted, so the version asserted below is the SECOND one.
    send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/chunks/{id}"),
        serde_json::json!({ "scope": { "env": "prod" } }),
    )
    .await;

    send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/chunks/{id}"),
        serde_json::json!({ "title": "Revised", "updateTag": "feature-x" }),
    )
    .await;

    let history = json_body(get(app, &cookie, &format!("/api/chunks/{id}/history")).await).await;
    let latest = &history[0];
    assert_eq!(
        latest["title"], "Original",
        "the snapshot is the pre-edit state"
    );
    assert_eq!(
        latest["alternatives"],
        serde_json::json!(["the road not taken"]),
        "alternatives must be recorded in history"
    );
    assert_eq!(
        latest["scope"],
        serde_json::json!({ "env": "prod" }),
        "scope must be recorded in history"
    );
    assert_eq!(
        latest["updateTag"], "feature-x",
        "the caller's updateTag must be recorded on the version"
    );
}

/// **Pins a Node behaviour, deliberately reproduced.** `POST /api/chunks`
/// declares `scope` in its route schema and then throws it away: Node's
/// `createChunk` parameter type omits the field and `createChunkRepo` never
/// passes it through (`chunk-mutations.ts:60-90`).
///
/// Left as-is after measuring — no first-party client sends `scope` on
/// create, so making Rust apply it would buy nothing real while making the
/// two stacks disagree. Setting a scope is `PATCH`'s job, asserted here in
/// the same test so the "it works over there" half is not just claimed.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_accepts_scope_and_drops_it_like_node(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;

    let res = create(
        app.clone(),
        &cookie,
        serde_json::json!({ "title": "T", "scope": { "env": "prod" } }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "an unused `scope` is accepted, not rejected"
    );
    let id = json_body(res).await["id"].as_str().unwrap().to_string();

    assert_eq!(
        detail(app.clone(), &cookie, &id).await["chunk"]["scope"],
        serde_json::json!({}),
        "scope supplied at create time is dropped, matching Node"
    );

    send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/chunks/{id}"),
        serde_json::json!({ "scope": { "env": "prod" } }),
    )
    .await;
    assert_eq!(
        detail(app, &cookie, &id).await["chunk"]["scope"],
        serde_json::json!({ "env": "prod" }),
        "PATCH is where scope lands"
    );
}

/// Node's route-schema limits are enforced, since Elysia validates them
/// before the handler and this port has no equivalent to inherit.
///
/// Enumerated across both bodies rather than sampled: growing the request
/// bodies without porting their constraints would swap one
/// silent-acceptance bug for another.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn write_bodies_enforce_nodes_route_schema_limits(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = json_body(create(app.clone(), &cookie, serde_json::json!({ "title": "T" })).await)
        .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let path = format!("/api/chunks/{id}");

    let long = |n: usize| "x".repeat(n);
    let many = |n: usize| (0..n).map(|i| format!("t{i}")).collect::<Vec<_>>();

    for (label, body) in [
        ("title over 200", serde_json::json!({ "title": long(201) })),
        (
            "content over 50000",
            serde_json::json!({ "title": "T", "content": long(50_001) }),
        ),
        (
            "type over 20",
            serde_json::json!({ "title": "T", "type": long(21) }),
        ),
        (
            "rationale over 5000",
            serde_json::json!({ "title": "T", "rationale": long(5_001) }),
        ),
        (
            "unknown origin",
            serde_json::json!({ "title": "T", "origin": "goblin" }),
        ),
        (
            "over 20 tags",
            serde_json::json!({ "title": "T", "tags": many(21) }),
        ),
        (
            "tag name over 50",
            serde_json::json!({ "title": "T", "tags": [long(51)] }),
        ),
    ] {
        assert_eq!(
            create(app.clone(), &cookie, body).await.status(),
            StatusCode::BAD_REQUEST,
            "POST must reject: {label}"
        );
    }

    for (label, body) in [
        (
            "unknown reviewStatus",
            serde_json::json!({ "reviewStatus": "vibes" }),
        ),
        (
            "summary over 500",
            serde_json::json!({ "summary": long(501) }),
        ),
        (
            "over 20 aliases",
            serde_json::json!({ "aliases": many(21) }),
        ),
        (
            "over 20 notAbout entries",
            serde_json::json!({ "notAbout": many(21) }),
        ),
        ("unknown origin", serde_json::json!({ "origin": "goblin" })),
    ] {
        assert_eq!(
            send(app.clone(), &cookie, "PATCH", &path, body)
                .await
                .status(),
            StatusCode::BAD_REQUEST,
            "PATCH must reject: {label}"
        );
    }

    // A rejected PATCH must not have written a version snapshot for an edit
    // that never happened — the validation runs before the snapshot.
    let history = json_body(get(app, &cookie, &format!("/api/chunks/{id}/history")).await).await;
    assert_eq!(
        history.as_array().unwrap().len(),
        0,
        "rejected PATCHes must leave no history behind"
    );
}
