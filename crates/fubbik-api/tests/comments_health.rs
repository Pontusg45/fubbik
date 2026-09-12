//! HTTP-level tests for the `comments`, `health` and `knowledge-health`
//! domains.
//!
//! Comments has an unusual shape worth testing carefully: **two different
//! owners**. Reading a thread is gated on owning the *chunk*; editing or
//! deleting a single comment is gated on having *written it*. A test that
//! only used one user would prove neither.

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
// Comments
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn comments_round_trip_oldest_first(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let chunk = a_chunk(app.clone(), &cookie, "T", "c").await;
    let path = format!("/api/chunks/{chunk}/comments");

    for text in ["first", "second"] {
        let res = send(
            app.clone(),
            &cookie,
            "POST",
            &path,
            serde_json::json!({ "content": text }),
        )
        .await;
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    let thread = json_body(get(app.clone(), &cookie, &path).await).await;
    let texts: Vec<&str> = thread
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["content"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["first", "second"], "oldest first");

    let id = thread[0]["id"].as_str().unwrap().to_string();
    let res = send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/comments/{id}"),
        serde_json::json!({ "content": "edited" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let edited = json_body(res).await;
    assert_eq!(edited["content"], "edited");
    assert_ne!(
        edited["updatedAt"], edited["createdAt"],
        "updatedAt must be bumped — Node relies on Drizzle's $onUpdate hook, \
         which has no database-level equivalent for Rust to inherit"
    );

    let res = send(
        app.clone(),
        &cookie,
        "DELETE",
        &format!("/api/comments/{id}"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Deleted");
    assert_eq!(
        json_body(get(app, &cookie, &path).await)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// The two-owner rule, both directions.
///
/// Bob may not read the thread on Alice's chunk. But once Alice's chunk is
/// shared with him — which this app has no mechanism for — the interesting
/// half is that a comment's *author* is what gates editing, so Alice cannot
/// edit a comment she did not write even on her own chunk. That second case
/// is constructed here by having Bob comment on his own chunk and Alice try
/// to edit it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn comment_reads_follow_the_chunk_and_writes_follow_the_author(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;

    let alices_chunk = a_chunk(app.clone(), &alice, "Alice's", "c").await;
    send(
        app.clone(),
        &alice,
        "POST",
        &format!("/api/chunks/{alices_chunk}/comments"),
        serde_json::json!({ "content": "mine" }),
    )
    .await;

    // Reading: gated on the chunk.
    let bobs_view = json_body(
        get(
            app.clone(),
            &bob,
            &format!("/api/chunks/{alices_chunk}/comments"),
        )
        .await,
    )
    .await;
    assert_eq!(
        bobs_view.as_array().unwrap().len(),
        0,
        "Bob must not read the thread on Alice's chunk"
    );

    // Posting: also gated on the chunk.
    let res = send(
        app.clone(),
        &bob,
        "POST",
        &format!("/api/chunks/{alices_chunk}/comments"),
        serde_json::json!({ "content": "intruding" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Editing: gated on authorship. Bob comments on his own chunk; Alice
    // cannot touch it even though she owns nothing about it either way.
    let bobs_chunk = a_chunk(app.clone(), &bob, "Bob's", "c").await;
    let bobs_comment = json_body(
        send(
            app.clone(),
            &bob,
            "POST",
            &format!("/api/chunks/{bobs_chunk}/comments"),
            serde_json::json!({ "content": "bob's words" }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();

    for (method, body) in [
        ("PATCH", serde_json::json!({ "content": "hijacked" })),
        ("DELETE", serde_json::Value::Null),
    ] {
        let res = send(
            app.clone(),
            &alice,
            method,
            &format!("/api/comments/{bobs_comment}"),
            body,
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::NOT_FOUND,
            "{method} on another author's comment must 404"
        );
    }

    // Bob's comment is untouched.
    let still =
        json_body(get(app, &bob, &format!("/api/chunks/{bobs_chunk}/comments")).await).await;
    assert_eq!(still[0]["content"], "bob's words");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_blank_comment_is_rejected(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let chunk = a_chunk(app.clone(), &cookie, "T", "c").await;

    let res = send(
        app,
        &cookie,
        "POST",
        &format!("/api/chunks/{chunk}/comments"),
        serde_json::json!({ "content": "   " }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// `/api/health` is deliberately unauthenticated — a probe with an expired
/// cookie must still report the service's state, not 401.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn health_answers_without_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let res = app
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["db"], "connected");
    assert!(
        body["ageAvailable"].is_boolean(),
        "ageAvailable is reported but must never gate the status — the graph \
         extension is optional"
    );
}

// ---------------------------------------------------------------------------
// Knowledge health
// ---------------------------------------------------------------------------

/// Each bucket is fed exactly one qualifying chunk and at least one that must
/// NOT qualify, so a query with a dropped predicate shows up as a count of 2
/// rather than passing quietly.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn knowledge_health_sorts_chunks_into_the_right_buckets(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;

    // Thin: under 100 chars. Also an orphan, since nothing connects it.
    let thin = a_chunk(app.clone(), &cookie, "Thin", "short").await;
    // Fat: over 100 chars, still an orphan.
    let fat = a_chunk(app.clone(), &cookie, "Fat", &"x".repeat(200)).await;

    let body = json_body(get(app.clone(), &cookie, "/api/health/knowledge").await).await;

    assert_eq!(body["thin"]["count"], 1, "only the short chunk is thin");
    assert_eq!(body["thin"]["chunks"][0]["id"], thin.as_str());
    assert_eq!(body["thin"]["chunks"][0]["contentLength"], 5);

    assert_eq!(body["orphans"]["count"], 2, "neither chunk is connected");

    // Connect them — both leave the orphan bucket.
    let res = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/connections",
        serde_json::json!({ "sourceId": thin, "targetId": fat, "relation": "related_to" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);

    let body = json_body(get(app.clone(), &cookie, "/api/health/knowledge").await).await;
    assert_eq!(
        body["orphans"]["count"], 0,
        "a connection in either direction clears orphan status"
    );
    assert_eq!(
        body["thin"]["count"], 1,
        "connecting does not make it thicker"
    );

    // Every bucket is present and shaped, even when empty.
    for key in ["orphans", "stale", "thin", "staleEmbeddings"] {
        assert!(
            body[key]["chunks"].is_array(),
            "`{key}.chunks` must be an array"
        );
        assert!(body[key]["count"].is_number());
    }
    assert!(
        body["fileRefs"]["refs"].is_array(),
        "fileRefs uses `refs`, not `chunks` — Node names this one differently"
    );
}

/// File references are reported with their chunk's title and type.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn knowledge_health_reports_file_refs(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let chunk = a_chunk(app.clone(), &cookie, "Documented", "c").await;

    send(
        app.clone(),
        &cookie,
        "PUT",
        &format!("/api/chunks/{chunk}/file-refs"),
        serde_json::json!([{ "path": "src/lib.rs", "relation": "implements" }]),
    )
    .await;

    let body = json_body(get(app, &cookie, "/api/health/knowledge").await).await;
    assert_eq!(body["fileRefs"]["count"], 1);
    let r = &body["fileRefs"]["refs"][0];
    assert_eq!(r["path"], "src/lib.rs");
    assert_eq!(r["relation"], "implements");
    assert_eq!(r["chunkTitle"], "Documented");
    assert_eq!(r["chunkId"], chunk.as_str());
}

/// Another user's chunks never appear, in any bucket.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn knowledge_health_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;
    a_chunk(app.clone(), &alice, "Alice's thin", "short").await;

    assert_eq!(
        json_body(get(app.clone(), &alice, "/api/health/knowledge").await).await["thin"]["count"],
        1,
        "Alice sees her own — otherwise the assertion below proves nothing"
    );
    let bobs = json_body(get(app, &bob, "/api/health/knowledge").await).await;
    assert_eq!(bobs["thin"]["count"], 0);
    assert_eq!(bobs["orphans"]["count"], 0);
}

/// The space filter means "in this space, **or** in no space at all" — global
/// chunks are everyone's business and appear under every space's view.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn the_space_filter_includes_global_chunks(pool: sqlx::PgPool) {
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
    let other = json_body(
        send(
            app.clone(),
            &cookie,
            "POST",
            "/api/spaces",
            serde_json::json!({ "name": "beta", "kind": "code" }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // One thin chunk in `alpha`, one in `beta`, one global.
    for (title, space_id) in [
        ("in-alpha", Some(space.clone())),
        ("in-beta", Some(other)),
        ("global", None),
    ] {
        let mut body = serde_json::json!({ "title": title, "content": "s" });
        if let Some(s) = space_id {
            body["spaceIds"] = serde_json::json!([s]);
        }
        send(app.clone(), &cookie, "POST", "/api/chunks", body).await;
    }

    let all = json_body(get(app.clone(), &cookie, "/api/health/knowledge").await).await;
    assert_eq!(all["thin"]["count"], 3, "unfiltered sees every chunk");

    let scoped = json_body(
        get(
            app,
            &cookie,
            &format!("/api/health/knowledge?spaceId={space}"),
        )
        .await,
    )
    .await;
    assert_eq!(
        scoped["thin"]["count"], 2,
        "the space's own chunk plus the global one — but NOT beta's"
    );
    let titles: Vec<&str> = scoped["thin"]["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"in-alpha"));
    assert!(titles.contains(&"global"));
    assert!(!titles.contains(&"in-beta"));
}
