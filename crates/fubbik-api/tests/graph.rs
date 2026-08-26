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

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn behavior_sync_projects_rules_for_every_user_and_is_idempotent(pool: sqlx::PgPool) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    let app = fubbik_api::router(state(pool.clone()));

    // A code_file vertex for the first user's rule to govern via a
    // cell-code link. `link_governs` matches with `ENDS WITH`, so the
    // cell-code ref only needs to be a suffix of this vertex id.
    fubbik_db::age::cypher(&pool, "CREATE (:code_file {id: 'src/auth/session.ts'})")
        .await
        .unwrap();

    let mut first_user_cookie = String::new();
    let mut first_user_matrix_id = String::new();
    let mut first_user_cell_id = String::new();
    let mut first_user_code_id = String::new();

    // Two DIFFERENT users, each with a matrix. Node syncs only the implicit
    // dev user (packages/api/src/startup.ts:52); this port syncs both, and
    // that divergence is the point of this assertion.
    for (i, (email, name, title)) in [
        ("a@b.test", "A", "Sessions expire"),
        ("c@d.test", "C", "Inputs are validated"),
    ]
    .into_iter()
    .enumerate()
    {
        let cookie = signup(app.clone(), email, name).await;
        let matrix = send(
            app.clone(),
            &cookie,
            "POST",
            "/api/matrices",
            serde_json::json!({ "name": "M", "layer": "invariant" }),
        )
        .await;
        let matrix_id = json_body(matrix).await["id"].as_str().unwrap().to_string();
        let rule = send(
            app.clone(),
            &cookie,
            "POST",
            &format!("/api/matrices/{matrix_id}/rules"),
            serde_json::json!({ "title": title, "category": "auth" }),
        )
        .await;

        if i == 0 {
            // Only the first user's rule is wired to actually govern code —
            // this is what makes the edge assertions below load-bearing.
            let rule_id = json_body(rule).await["id"].as_str().unwrap().to_string();

            let dimension = send(
                app.clone(),
                &cookie,
                "POST",
                &format!("/api/matrices/{matrix_id}/dimensions"),
                serde_json::json!({ "name": "Auth" }),
            )
            .await;
            let dimension_id = json_body(dimension).await["id"]
                .as_str()
                .unwrap()
                .to_string();

            let cell = send(
                app.clone(),
                &cookie,
                "PUT",
                &format!("/api/matrices/{matrix_id}/cells"),
                serde_json::json!({ "ruleId": rule_id, "dimensionId": dimension_id }),
            )
            .await;
            let cell_id = json_body(cell).await["cell"]["id"]
                .as_str()
                .unwrap()
                .to_string();

            let code = send(
                app.clone(),
                &cookie,
                "POST",
                &format!("/api/matrices/{matrix_id}/cells/{cell_id}/code"),
                serde_json::json!({ "kind": "file", "ref": "auth/session.ts" }),
            )
            .await;
            let code_id = json_body(code).await["id"].as_str().unwrap().to_string();

            first_user_cookie = cookie;
            first_user_matrix_id = matrix_id;
            first_user_cell_id = cell_id;
            first_user_code_id = code_id;
        }
    }

    let first = fubbik_api::graph::sync::sync_once(&pool).await.unwrap();
    assert_eq!(first, 2, "both users' rules must be projected");

    let second = fubbik_api::graph::sync::sync_once(&pool).await.unwrap();
    assert_eq!(second, 2);

    let vertices = fubbik_db::age::list_behavior_rule_vertices(&pool)
        .await
        .unwrap();
    assert_eq!(
        vertices.len(),
        2,
        "a second sweep must not duplicate vertices"
    );
    let titles: Vec<&str> = vertices.iter().map(|v| v.title.as_str()).collect();
    assert!(titles.contains(&"Sessions expire"));
    assert!(titles.contains(&"Inputs are validated"));

    // The claim the brief's original test never made: the one cell-code
    // link must actually have produced a governs edge, and re-sweeping
    // identical data must not duplicate it. A count that stays 0 across
    // both sweeps (the reviewer's mutation-tested vacuity) would pass this
    // half trivially, so assert non-zero first.
    let edges = fubbik_db::age::list_governs_edges(&pool).await.unwrap();
    assert_eq!(
        edges.len(),
        1,
        "the one cell-code link must produce exactly one governs edge, and \
         two sweeps of identical data must not duplicate it"
    );

    // Now change what the sweep should produce: remove the underlying
    // cell-code link entirely, then sweep again. This is the assertion that
    // actually requires `delete_governs_edges` to run — with MERGE alone
    // (the anti-pattern the brief warns against), a stale edge for a
    // link that no longer exists would never be cleaned up, and this
    // assertion would fail without the delete-before-relink step.
    send(
        app.clone(),
        &first_user_cookie,
        "DELETE",
        &format!(
            "/api/matrices/{first_user_matrix_id}/cells/{first_user_cell_id}/code/{first_user_code_id}"
        ),
        serde_json::Value::Null,
    )
    .await;

    let third = fubbik_api::graph::sync::sync_once(&pool).await.unwrap();
    assert_eq!(
        third, 2,
        "rule vertices are unaffected by removing a code link"
    );

    let edges_after_removal = fubbik_db::age::list_governs_edges(&pool).await.unwrap();
    assert!(
        edges_after_removal.is_empty(),
        "a stale governs edge for a deleted cell-code link must be removed on the next sweep"
    );
}
