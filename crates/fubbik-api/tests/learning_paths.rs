//! HTTP-level tests for `/api/learning-paths`.
//!
//! Membership is a `jsonb` array on the row rather than a join table, so
//! there is no foreign key to enforce that the chunk ids belong to the
//! caller — Node stores whatever it is given. The checks below are the only
//! place that can be enforced, which makes them the interesting part.

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

async fn a_chunk(app: axum::Router, cookie: &str, title: &str) -> String {
    let res = send(
        app,
        cookie,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": title, "content": "c" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    json_body(res).await["id"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn learning_paths_round_trip_and_preserve_order(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let a = a_chunk(app.clone(), &cookie, "A").await;
    let b = a_chunk(app.clone(), &cookie, "B").await;
    let c = a_chunk(app.clone(), &cookie, "C").await;

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/learning-paths",
        serde_json::json!({
            "title": "  Getting started  ",
            "description": "read these in order",
            "chunkIds": [c, a, b]
        }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created = json_body(res).await;
    assert_eq!(created["title"], "Getting started", "title is trimmed");
    assert_eq!(
        created["chunkIds"],
        serde_json::json!([c, a, b]),
        "order is the data — it must survive exactly as given"
    );
    let id = created["id"].as_str().unwrap().to_string();

    let fetched =
        json_body(get(app.clone(), &cookie, &format!("/api/learning-paths/{id}")).await).await;
    assert_eq!(fetched["chunkIds"], serde_json::json!([c, a, b]));

    // Reordering is just a new list.
    let updated = json_body(
        send(
            app.clone(),
            &cookie,
            "PATCH",
            &format!("/api/learning-paths/{id}"),
            serde_json::json!({ "chunkIds": [a, b, c] }),
        )
        .await,
    )
    .await;
    assert_eq!(updated["chunkIds"], serde_json::json!([a, b, c]));
    assert_ne!(
        updated["updatedAt"], updated["createdAt"],
        "updatedAt must be bumped — the list is ordered by it, so an edit \
         that did not bump would sort as untouched"
    );

    let res = send(
        app.clone(),
        &cookie,
        "DELETE",
        &format!("/api/learning-paths/{id}"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Deleted");
    assert_eq!(
        get(app, &cookie, &format!("/api/learning-paths/{id}"))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

/// `chunkIds: []` empties the path; omitting the key leaves it alone. The
/// distinction is what a naive "is the list non-empty?" check destroys.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn an_empty_chunk_list_clears_the_path_but_an_absent_one_does_not(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let a = a_chunk(app.clone(), &cookie, "A").await;

    let id = json_body(
        send(
            app.clone(),
            &cookie,
            "POST",
            "/api/learning-paths",
            serde_json::json!({ "title": "P", "chunkIds": [a] }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let path = format!("/api/learning-paths/{id}");

    // Absent — untouched.
    let out = json_body(
        send(
            app.clone(),
            &cookie,
            "PATCH",
            &path,
            serde_json::json!({ "title": "Renamed" }),
        )
        .await,
    )
    .await;
    assert_eq!(
        out["chunkIds"].as_array().unwrap().len(),
        1,
        "omitting chunkIds must leave the list alone"
    );

    // Explicit empty — cleared.
    let out = json_body(
        send(
            app,
            &cookie,
            "PATCH",
            &path,
            serde_json::json!({ "chunkIds": [] }),
        )
        .await,
    )
    .await;
    assert_eq!(
        out["chunkIds"].as_array().unwrap().len(),
        0,
        "`chunkIds: []` must empty the path"
    );
}

/// **Divergence from Node.** There is no FK on `chunk_ids`, so Node stores
/// whatever ids it is handed — including another user's, or ones that do not
/// exist. Both directions rejected here, on create and on update.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_path_cannot_contain_chunks_the_caller_does_not_own(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;
    let hers = a_chunk(app.clone(), &alice, "Alice's").await;
    let his = a_chunk(app.clone(), &bob, "Bob's").await;

    for (label, ids) in [
        ("a foreign chunk", serde_json::json!([his])),
        ("a nonexistent chunk", serde_json::json!(["no-such-chunk"])),
        ("one good and one foreign", serde_json::json!([hers, his])),
    ] {
        let res = send(
            app.clone(),
            &alice,
            "POST",
            "/api/learning-paths",
            serde_json::json!({ "title": "P", "chunkIds": ids }),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::BAD_REQUEST,
            "create must reject: {label}"
        );
    }

    // Her own chunk works — otherwise the rejections above prove nothing.
    let id = json_body(
        send(
            app.clone(),
            &alice,
            "POST",
            "/api/learning-paths",
            serde_json::json!({ "title": "P", "chunkIds": [hers] }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // And the same guard applies to PATCH — reported as 400, not 404, since
    // the path itself is hers.
    let res = send(
        app.clone(),
        &alice,
        "PATCH",
        &format!("/api/learning-paths/{id}"),
        serde_json::json!({ "chunkIds": [his] }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let still = json_body(get(app, &alice, &format!("/api/learning-paths/{id}")).await).await;
    assert_eq!(
        still["chunkIds"],
        serde_json::json!([hers]),
        "a rejected PATCH must leave the list untouched"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn learning_paths_are_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;

    let id = json_body(
        send(
            app.clone(),
            &alice,
            "POST",
            "/api/learning-paths",
            serde_json::json!({ "title": "Alice's", "chunkIds": [] }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let path = format!("/api/learning-paths/{id}");

    assert!(
        json_body(get(app.clone(), &bob, "/api/learning-paths").await)
            .await
            .as_array()
            .unwrap()
            .is_empty()
    );
    for (method, body) in [
        ("GET", serde_json::Value::Null),
        ("PATCH", serde_json::json!({ "title": "Hijacked" })),
        ("DELETE", serde_json::Value::Null),
    ] {
        let res = send(app.clone(), &bob, method, &path, body).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{method} {path}");
    }

    assert_eq!(
        json_body(get(app, &alice, &path).await).await["title"],
        "Alice's",
        "Alice's path survives"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_blank_title_is_rejected(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let res = send(
        app,
        &cookie,
        "POST",
        "/api/learning-paths",
        serde_json::json!({ "title": "   ", "chunkIds": [] }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
