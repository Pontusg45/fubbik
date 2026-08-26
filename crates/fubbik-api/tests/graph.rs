//! HTTP-level tests for `GET /api/graph`.

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

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn graph_requires_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let res = app
        .oneshot(Request::get("/api/graph").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn graph_returns_the_seven_documented_fields(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let res = send(app, &cookie, "GET", "/api/graph", serde_json::Value::Null).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;

    for field in [
        "chunks",
        "connections",
        "chunkTags",
        "tagTypes",
        "chunkCodebases",
        "behaviorRules",
        "governsEdges",
    ] {
        assert!(body.get(field).is_some(), "missing field {field}");
        assert!(body[field].is_array(), "{field} must be an array");
    }

    // The six fields Node returns and nothing reads are deliberately absent —
    // see the spec. Their absence is the documentation.
    for dropped in [
        "communities",
        "bridges",
        "codeFiles",
        "codeSymbols",
        "concepts",
        "coRefEdges",
    ] {
        assert!(
            body.get(dropped).is_none(),
            "{dropped} should not be served"
        );
    }
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn graph_space_scoping_actually_filters(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let space = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/spaces",
        serde_json::json!({ "name": "Target", "kind": "code" }),
    )
    .await;
    let space_id = json_body(space).await["id"].as_str().unwrap().to_string();

    let scoped = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": "In target", "content": "x", "type": "note",
                            "spaceIds": [space_id] }),
    )
    .await;
    assert_eq!(scoped.status(), StatusCode::CREATED);

    let other_space = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/spaces",
        serde_json::json!({ "name": "Other", "kind": "code" }),
    )
    .await;
    let other_id = json_body(other_space).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": "In other", "content": "x", "type": "note",
                            "spaceIds": [other_id] }),
    )
    .await;

    let res = send(
        app,
        &cookie,
        "GET",
        &format!("/api/graph?spaceId={space_id}"),
        serde_json::Value::Null,
    )
    .await;
    let body = json_body(res).await;
    let titles: Vec<&str> = body["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();

    // This is the bug being fixed. On Node the param is named `codebaseId`
    // (packages/api/src/graph/routes.ts:18), Elysia strips the unknown
    // `spaceId`, and BOTH titles come back.
    assert!(titles.contains(&"In target"));
    assert!(
        !titles.contains(&"In other"),
        "spaceId must actually scope the graph"
    );
}
