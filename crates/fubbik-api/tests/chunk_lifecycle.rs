//! HTTP-level tests for the chunk lifecycle: archive, restore, the archived
//! list, bulk-update, bulk delete, and merge.
//!
//! Merge is the one worth reading closely. It re-parents six tables in a
//! transaction, three of which carry unique constraints that a naive
//! re-parent would violate, and it is **broken in Node** — its raw SQL names
//! a `favorite` table that does not exist (the table is `user_favorite`), so
//! the statement raises inside the transaction and rolls the whole merge
//! back. The tests below therefore describe intended behaviour rather than
//! observed Node behaviour; see `chunk::merge`'s doc comment.

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
    assert_eq!(res.status(), StatusCode::OK);
    res.headers()
        .get("set-cookie")
        .unwrap()
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

async fn send(
    app: axum::Router,
    cookie: &str,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookie);
    if !body.is_null() {
        req = req.header("content-type", "application/json");
    }
    let b = if body.is_null() {
        Body::empty()
    } else {
        Body::from(body.to_string())
    };
    app.oneshot(req.body(b).unwrap()).await.unwrap()
}

async fn get(app: axum::Router, cookie: &str, path: &str) -> axum::response::Response {
    send(app, cookie, "GET", path, serde_json::Value::Null).await
}

async fn a_chunk(app: axum::Router, cookie: &str, title: &str, content: &str) -> String {
    let res = send(
        app,
        cookie,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": title, "content": content }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    json_body(res).await["id"].as_str().unwrap().to_string()
}

// ---------------------------------------------------------------------------
// Archive / restore
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn archive_hides_from_the_list_and_restore_brings_it_back(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = a_chunk(app.clone(), &cookie, "Doomed", "c").await;

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/chunks/{id}/archive"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Archived");

    let archived = json_body(get(app.clone(), &cookie, "/api/chunks/archived").await).await;
    assert_eq!(archived.as_array().unwrap().len(), 1);
    assert_eq!(archived[0]["id"], id.as_str());
    assert!(archived[0]["archivedAt"].is_string());

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/chunks/{id}/restore"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Restored");

    assert_eq!(
        json_body(get(app.clone(), &cookie, "/api/chunks/archived").await)
            .await
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let detail = json_body(get(app, &cookie, &format!("/api/chunks/{id}")).await).await;
    assert_eq!(detail["chunk"]["archivedAt"], serde_json::Value::Null);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn archive_restore_and_archived_are_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;
    let id = a_chunk(app.clone(), &alice, "Alice's", "c").await;

    for path in [
        format!("/api/chunks/{id}/archive"),
        format!("/api/chunks/{id}/restore"),
    ] {
        let res = send(app.clone(), &bob, "POST", &path, serde_json::Value::Null).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{path}");
    }

    send(
        app.clone(),
        &alice,
        "POST",
        &format!("/api/chunks/{id}/archive"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(
        json_body(get(app, &bob, "/api/chunks/archived").await)
            .await
            .as_array()
            .unwrap()
            .len(),
        0,
        "Bob must not see Alice's archived chunks"
    );
}

// ---------------------------------------------------------------------------
// Bulk
// ---------------------------------------------------------------------------

/// Every action, enumerated. Each is a separate branch in one match, so a
/// mistake in one is invisible from testing another.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn bulk_update_applies_each_action(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let a = a_chunk(app.clone(), &cookie, "A", "c").await;
    let b = a_chunk(app.clone(), &cookie, "B", "c").await;
    let ids = serde_json::json!([a, b]);

    async fn bulk(
        app: axum::Router,
        cookie: &str,
        ids: &serde_json::Value,
        action: &str,
        value: Option<&str>,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({ "ids": ids, "action": action });
        if let Some(v) = value {
            body["value"] = serde_json::json!(v);
        }
        let res = send(app, cookie, "POST", "/api/chunks/bulk-update", body).await;
        assert_eq!(res.status(), StatusCode::OK, "action {action}");
        json_body(res).await
    }

    // add_tags, then remove one of the two
    bulk(app.clone(), &cookie, &ids, "add_tags", Some("alpha, beta")).await;
    let detail = json_body(get(app.clone(), &cookie, &format!("/api/chunks/{a}")).await).await;
    let mut tags: Vec<&str> = detail["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    tags.sort_unstable();
    assert_eq!(tags, ["alpha", "beta"]);

    bulk(app.clone(), &cookie, &ids, "remove_tags", Some("alpha")).await;
    let detail = json_body(get(app.clone(), &cookie, &format!("/api/chunks/{a}")).await).await;
    let tags: Vec<&str> = detail["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(tags, ["beta"], "only the named tag is removed");

    bulk(app.clone(), &cookie, &ids, "set_type", Some("reference")).await;
    bulk(
        app.clone(),
        &cookie,
        &ids,
        "set_review_status",
        Some("reviewed"),
    )
    .await;
    let detail = json_body(get(app.clone(), &cookie, &format!("/api/chunks/{a}")).await).await;
    assert_eq!(detail["chunk"]["type"], "reference");
    assert_eq!(detail["chunk"]["reviewStatus"], "reviewed");

    // set_codebase with a real space, then null to clear
    let space = json_body(
        send(
            app.clone(),
            &cookie,
            "POST",
            "/api/spaces",
            serde_json::json!({ "name": "s", "kind": "code" }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    bulk(app.clone(), &cookie, &ids, "set_codebase", Some(&space)).await;
    let detail = json_body(get(app.clone(), &cookie, &format!("/api/chunks/{a}")).await).await;
    assert_eq!(detail["spaces"].as_array().unwrap().len(), 1);

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks/bulk-update",
        serde_json::json!({ "ids": ids, "action": "set_codebase", "value": null }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let detail = json_body(get(app.clone(), &cookie, &format!("/api/chunks/{a}")).await).await;
    assert_eq!(
        detail["spaces"].as_array().unwrap().len(),
        0,
        "a null value clears the chunk's spaces"
    );

    // archive, then delete
    let out = bulk(app.clone(), &cookie, &ids, "archive", None).await;
    assert_eq!(out["updated"], 2);
    assert_eq!(
        json_body(get(app.clone(), &cookie, "/api/chunks/archived").await)
            .await
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let out = bulk(app.clone(), &cookie, &ids, "delete", None).await;
    assert_eq!(out["updated"], 2);
    assert_eq!(
        get(app, &cookie, &format!("/api/chunks/{a}"))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

/// One foreign id fails the whole batch, changing nothing — ownership is
/// validated for every id before any write.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_batch_with_one_foreign_id_writes_nothing(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;
    let mine = a_chunk(app.clone(), &bob, "Bob's own", "c").await;
    let hers = a_chunk(app.clone(), &alice, "Alice's", "c").await;

    let res = send(
        app.clone(),
        &bob,
        "POST",
        "/api/chunks/bulk-update",
        serde_json::json!({ "ids": [mine, hers], "action": "set_type", "value": "schema" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Bob's own chunk is untouched — the batch is all-or-nothing.
    let detail = json_body(get(app, &bob, &format!("/api/chunks/{mine}")).await).await;
    assert_eq!(
        detail["chunk"]["type"], "note",
        "a rejected batch must not partially apply"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn bulk_update_rejects_bad_actions_and_values(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = a_chunk(app.clone(), &cookie, "A", "c").await;
    let ids = serde_json::json!([id]);

    for (label, body) in [
        (
            "unknown action",
            serde_json::json!({ "ids": ids, "action": "vandalise" }),
        ),
        (
            "add_tags with no value",
            serde_json::json!({ "ids": ids, "action": "add_tags" }),
        ),
        (
            "add_tags whose value is only separators",
            serde_json::json!({ "ids": ids, "action": "add_tags", "value": " , , " }),
        ),
        (
            "set_review_status with a bad value",
            serde_json::json!({ "ids": ids, "action": "set_review_status", "value": "vibes" }),
        ),
        (
            "over 100 ids",
            serde_json::json!({
                "ids": (0..101).map(|i| format!("id{i}")).collect::<Vec<_>>(),
                "action": "archive"
            }),
        ),
    ] {
        let res = send(
            app.clone(),
            &cookie,
            "POST",
            "/api/chunks/bulk-update",
            body,
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::BAD_REQUEST,
            "must reject: {label}"
        );
    }
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn bulk_delete_only_removes_the_callers_chunks(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;
    let hers = a_chunk(app.clone(), &alice, "Alice's", "c").await;
    let his = a_chunk(app.clone(), &bob, "Bob's", "c").await;

    let res = send(
        app.clone(),
        &bob,
        "DELETE",
        "/api/chunks/bulk",
        serde_json::json!({ "ids": [his, hers] }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await["deleted"],
        1,
        "only Bob's own chunk is counted and removed"
    );
    assert_eq!(
        get(app, &alice, &format!("/api/chunks/{hers}"))
            .await
            .status(),
        StatusCode::OK,
        "Alice's chunk survives"
    );
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// The full re-parent: tags, spaces, connections, file refs, applies-to and
/// favourites all move, content is appended, and the source is gone.
///
/// Each side is given something the other has and something it does not, so
/// a merge that dropped one side (or double-counted) shows up as a wrong
/// count rather than passing.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn merge_reparents_everything_and_deletes_the_source(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;

    let source = json_body(
        send(
            app.clone(),
            &cookie,
            "POST",
            "/api/chunks",
            serde_json::json!({
                "title": "Source", "content": "source body",
                "tags": ["shared", "only-source"]
            }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let target = json_body(
        send(
            app.clone(),
            &cookie,
            "POST",
            "/api/chunks",
            serde_json::json!({
                "title": "Target", "content": "target body",
                "tags": ["shared", "only-target"]
            }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let neighbour = a_chunk(app.clone(), &cookie, "Neighbour", "c").await;

    // The source has a connection, a file ref and an applies-to pattern.
    send(
        app.clone(),
        &cookie,
        "POST",
        "/api/connections",
        serde_json::json!({ "sourceId": source, "targetId": neighbour, "relation": "related_to" }),
    )
    .await;
    send(
        app.clone(),
        &cookie,
        "PUT",
        &format!("/api/chunks/{source}/file-refs"),
        serde_json::json!([{ "path": "src/from-source.rs", "relation": "documents" }]),
    )
    .await;
    send(
        app.clone(),
        &cookie,
        "PUT",
        &format!("/api/chunks/{source}/applies-to"),
        serde_json::json!([{ "pattern": "src/**" }]),
    )
    .await;
    // And a favourite — the table Node's merge names wrongly.
    send(
        app.clone(),
        &cookie,
        "POST",
        "/api/favorites",
        serde_json::json!({ "chunkId": source }),
    )
    .await;

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks/merge",
        serde_json::json!({ "sourceId": source, "targetId": target }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK, "merge must succeed");
    let merged = json_body(res).await;
    assert_eq!(merged["id"], target.as_str());
    assert!(
        merged["content"]
            .as_str()
            .unwrap()
            .contains("## Merged from \"Source\""),
        "the source body is appended under a heading"
    );
    assert!(merged["content"].as_str().unwrap().contains("source body"));
    assert!(merged["content"].as_str().unwrap().contains("target body"));

    // Source is gone.
    assert_eq!(
        get(app.clone(), &cookie, &format!("/api/chunks/{source}"))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    let detail = json_body(get(app.clone(), &cookie, &format!("/api/chunks/{target}")).await).await;
    let mut tags: Vec<&str> = detail["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    tags.sort_unstable();
    assert_eq!(
        tags,
        ["only-source", "only-target", "shared"],
        "the shared tag is carried once, not duplicated"
    );
    assert_eq!(
        detail["connections"].as_array().unwrap().len(),
        1,
        "the source's connection is repointed at the target"
    );
    assert_eq!(detail["fileReferences"][0]["path"], "src/from-source.rs");
    assert_eq!(detail["appliesTo"][0]["pattern"], "src/**");

    // The favourite moved — this is the statement that always failed in Node.
    let favs = json_body(get(app, &cookie, "/api/favorites").await).await;
    let fav_ids: Vec<&str> = favs
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["chunkId"].as_str())
        .collect();
    assert!(
        fav_ids.contains(&target.as_str()),
        "the source's favourite must be carried over to the target"
    );
}

/// Merging when both chunks are already connected to each other would create
/// a self-loop; it must be removed.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn merging_two_connected_chunks_leaves_no_self_loop(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let source = a_chunk(app.clone(), &cookie, "Source", "s").await;
    let target = a_chunk(app.clone(), &cookie, "Target", "t").await;

    send(
        app.clone(),
        &cookie,
        "POST",
        "/api/connections",
        serde_json::json!({ "sourceId": source, "targetId": target, "relation": "related_to" }),
    )
    .await;

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks/merge",
        serde_json::json!({ "sourceId": source, "targetId": target }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let detail = json_body(get(app, &cookie, &format!("/api/chunks/{target}")).await).await;
    assert_eq!(
        detail["connections"].as_array().unwrap().len(),
        0,
        "the edge between them becomes a self-loop and must be deleted"
    );
}

/// Merging twice must not duplicate the appended body.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn merging_identical_content_does_not_append_it(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let source = a_chunk(app.clone(), &cookie, "Source", "same body").await;
    let target = a_chunk(app.clone(), &cookie, "Target", "same body").await;

    let merged = json_body(
        send(
            app,
            &cookie,
            "POST",
            "/api/chunks/merge",
            serde_json::json!({ "sourceId": source, "targetId": target }),
        )
        .await,
    )
    .await;
    assert_eq!(
        merged["content"], "same body",
        "content already present in the target is not appended again"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn merge_rejects_self_and_foreign_chunks(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;
    let hers = a_chunk(app.clone(), &alice, "Alice's", "c").await;
    let his = a_chunk(app.clone(), &bob, "Bob's", "c").await;

    let res = send(
        app.clone(),
        &alice,
        "POST",
        "/api/chunks/merge",
        serde_json::json!({ "sourceId": hers, "targetId": hers }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "self-merge");

    let res = send(
        app.clone(),
        &bob,
        "POST",
        "/api/chunks/merge",
        serde_json::json!({ "sourceId": hers, "targetId": his }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND, "foreign source");

    // Alice's chunk survives Bob's attempt.
    assert_eq!(
        get(app, &alice, &format!("/api/chunks/{hers}"))
            .await
            .status(),
        StatusCode::OK
    );
}
