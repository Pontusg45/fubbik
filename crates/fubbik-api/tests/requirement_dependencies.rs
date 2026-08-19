//! HTTP-level tests for `requirements`' dependency sub-resource routes.

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
    serde_json::from_slice(&body).unwrap()
}

async fn create_requirement(app: axum::Router, cookie: &str, title: &str) -> String {
    let steps = serde_json::json!([{"keyword": "given", "text": "x"}, {"keyword": "when", "text": "y"}, {"keyword": "then", "text": "z"}]);
    let res = app
        .oneshot(
            Request::post("/api/requirements")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(
                    serde_json::json!({"title": title, "steps": steps}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    json_body(res).await["requirement"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn add_dependency(
    app: axum::Router,
    cookie: &str,
    id: &str,
    depends_on_id: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::post(format!("/api/requirements/{id}/dependencies"))
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(
                serde_json::json!({"dependsOnId": depends_on_id}).to_string(),
            ))
            .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_get_and_remove_round_trip(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-dep@b.test", "Alice").await;
    let a = create_requirement(app.clone(), &cookie, "A").await;
    let b = create_requirement(app.clone(), &cookie, "B").await;

    let res = add_dependency(app.clone(), &cookie, &a, &b).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({"message": "Dependency added"})
    );

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/requirements/{a}/dependencies"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let deps = json_body(res).await;
    assert_eq!(deps["dependsOn"][0]["id"], b);
    assert_eq!(deps["dependedOnBy"].as_array().unwrap().len(), 0);

    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/requirements/{a}/dependencies/{b}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({"message": "Dependency removed"})
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_rejects_a_cycle(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cycle@b.test", "Alice").await;
    let a = create_requirement(app.clone(), &cookie, "A").await;
    let b = create_requirement(app.clone(), &cookie, "B").await;
    let c = create_requirement(app.clone(), &cookie, "C").await;

    assert_eq!(
        add_dependency(app.clone(), &cookie, &a, &b).await.status(),
        StatusCode::CREATED
    );
    assert_eq!(
        add_dependency(app.clone(), &cookie, &b, &c).await.status(),
        StatusCode::CREATED
    );

    // c -> a would close the a -> b -> c -> a cycle.
    let res = add_dependency(app.clone(), &cookie, &c, &a).await;
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "adding a dependency that closes a cycle must be rejected"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_rejects_a_self_dependency(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-self-dep@b.test", "Alice").await;
    let a = create_requirement(app.clone(), &cookie, "A").await;

    let res = add_dependency(app.clone(), &cookie, &a, &a).await;
    assert!(
        res.status().is_client_error() || res.status().is_server_error(),
        "a requirement cannot depend on itself, matching Node's DB-constraint-only guard"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_dependency_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-dep-owner@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-dep-owner@b.test", "Bob").await;
    let a = create_requirement(app.clone(), &alice_cookie, "Alice's").await;
    let b = create_requirement(app.clone(), &alice_cookie, "Alice's other").await;

    let res = add_dependency(app.clone(), &bob_cookie, &a, &b).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "Bob must not be able to add a dependency on Alice's requirement"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn dependency_graph_includes_current_and_transitive_nodes(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-graph@b.test", "Alice").await;
    let a = create_requirement(app.clone(), &cookie, "A").await;
    let b = create_requirement(app.clone(), &cookie, "B").await;
    assert_eq!(
        add_dependency(app.clone(), &cookie, &a, &b).await.status(),
        StatusCode::CREATED
    );

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/requirements/{a}/dependencies/graph"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let graph = json_body(res).await;
    let node_ids: Vec<String> = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect();
    assert!(node_ids.contains(&a) && node_ids.contains(&b));
    let current = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == a)
        .unwrap();
    assert_eq!(current["isCurrent"], true);
    assert_eq!(
        graph["edges"][0],
        serde_json::json!({"source": a, "target": b})
    );
}
