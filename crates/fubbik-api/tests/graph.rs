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

/// `esc_cypher` escapes `\` and `'` but not `$$`, and the Cypher is embedded
/// in a `$$`-dollar-quoted SQL block (`age.rs`'s safety note). A rule whose
/// title contains `$$` therefore terminates that block early and the
/// `upsert_behavior_rule` statement fails to parse. `sync_once` orders rules
/// by `r.id ASC`, so if that one failure aborted the sweep, every rule
/// ordered after the bad one would never be projected — forever, on every
/// tick. This proves the sweep instead skips the bad rule and keeps going.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn behavior_sync_skips_a_rule_that_fails_and_still_projects_the_rest(pool: sqlx::PgPool) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let matrix = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/matrices",
        serde_json::json!({ "name": "M", "layer": "invariant" }),
    )
    .await;
    let matrix_id = json_body(matrix).await["id"].as_str().unwrap().to_string();

    let rule_a = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/matrices/{matrix_id}/rules"),
        serde_json::json!({ "title": "placeholder a" }),
    )
    .await;
    let rule_a_id = json_body(rule_a).await["id"].as_str().unwrap().to_string();

    let rule_b = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/matrices/{matrix_id}/rules"),
        serde_json::json!({ "title": "placeholder b" }),
    )
    .await;
    let rule_b_id = json_body(rule_b).await["id"].as_str().unwrap().to_string();

    // `sync_once` walks rules ordered by `r.id ASC`. Work out which of the
    // two ids sorts first and give THAT one the poisoned title, so the
    // failure lands ahead of the rule whose survival we're asserting.
    let (poisoned_id, poisoned_title, survivor_id, survivor_title) = if rule_a_id < rule_b_id {
        (
            rule_a_id,
            "broken$$title",
            rule_b_id,
            "Inputs are validated",
        )
    } else {
        (
            rule_b_id,
            "broken$$title",
            rule_a_id,
            "Inputs are validated",
        )
    };

    let patch = send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/matrices/{matrix_id}/rules/{poisoned_id}"),
        serde_json::json!({ "title": poisoned_title }),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::OK);

    let patch = send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/matrices/{matrix_id}/rules/{survivor_id}"),
        serde_json::json!({ "title": survivor_title }),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::OK);

    let synced = fubbik_api::graph::sync::sync_once(&pool).await.unwrap();
    assert_eq!(
        synced, 1,
        "only the un-poisoned rule counts as successfully synced"
    );

    let vertices = fubbik_db::age::list_behavior_rule_vertices(&pool)
        .await
        .unwrap();
    let titles: Vec<&str> = vertices.iter().map(|v| v.title.as_str()).collect();
    assert!(
        titles.contains(&survivor_title),
        "the rule after the poisoned one in id order must still be projected, got {titles:?}"
    );
    assert!(
        !titles.iter().any(|t| t.contains("broken")),
        "the poisoned rule must not have produced a vertex"
    );
}

/// `graph::sync::sync_once` deliberately sweeps every user's matrices into
/// the AGE graph — the correct behaviour, and a real divergence from Node,
/// which only ever wrote the implicit dev user's rules
/// (`packages/api/src/startup.ts:52`) and so never faced this seam. But
/// `age::list_behavior_rule_vertices` / `age::list_governs_edges` have no
/// user id to filter on: AGE vertices don't carry one. Without a filter at
/// the service layer, user A's `GET /api/graph` would return user B's
/// behavior-rule titles and matrix ids. This is the assertion the review
/// flagged as missing.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn graph_does_not_leak_another_users_behavior_rules(pool: sqlx::PgPool) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    let app = fubbik_api::router(state(pool.clone()));

    let cookie_a = signup(app.clone(), "a@b.test", "A").await;
    let matrix_a = send(
        app.clone(),
        &cookie_a,
        "POST",
        "/api/matrices",
        serde_json::json!({ "name": "M-A", "layer": "invariant" }),
    )
    .await;
    let matrix_a_id = json_body(matrix_a).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    send(
        app.clone(),
        &cookie_a,
        "POST",
        &format!("/api/matrices/{matrix_a_id}/rules"),
        serde_json::json!({ "title": "A's secret rule" }),
    )
    .await;

    let cookie_b = signup(app.clone(), "c@d.test", "C").await;
    let matrix_b = send(
        app.clone(),
        &cookie_b,
        "POST",
        "/api/matrices",
        serde_json::json!({ "name": "M-B", "layer": "invariant" }),
    )
    .await;
    let matrix_b_id = json_body(matrix_b).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let rule_b = send(
        app.clone(),
        &cookie_b,
        "POST",
        &format!("/api/matrices/{matrix_b_id}/rules"),
        serde_json::json!({ "title": "B's secret rule" }),
    )
    .await;
    let rule_b_id = json_body(rule_b).await["id"].as_str().unwrap().to_string();

    // A `code_file` vertex for B's rule to govern — `link_governs` only
    // MATCHes existing code vertices, it never creates them.
    fubbik_db::age::cypher(&pool, "CREATE (:code_file {id: 'src/auth/session.ts'})")
        .await
        .unwrap();

    // Wire B's rule to code so the sweep produces a `governs` edge for it.
    // Without this the graph holds no edges at all, and the `governsEdges`
    // half of the ownership filter could be deleted outright with this test
    // still passing — the leak would only be caught for rule titles.
    let dimension_b = send(
        app.clone(),
        &cookie_b,
        "POST",
        &format!("/api/matrices/{matrix_b_id}/dimensions"),
        serde_json::json!({ "name": "Auth" }),
    )
    .await;
    let dimension_b_id = json_body(dimension_b).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let cell_b = send(
        app.clone(),
        &cookie_b,
        "PUT",
        &format!("/api/matrices/{matrix_b_id}/cells"),
        serde_json::json!({ "ruleId": rule_b_id, "dimensionId": dimension_b_id }),
    )
    .await;
    let cell_b_id = json_body(cell_b).await["cell"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    send(
        app.clone(),
        &cookie_b,
        "POST",
        &format!("/api/matrices/{matrix_b_id}/cells/{cell_b_id}/code"),
        serde_json::json!({ "kind": "file", "ref": "auth/session.ts" }),
    )
    .await;

    let synced = fubbik_api::graph::sync::sync_once(&pool).await.unwrap();
    assert_eq!(synced, 2, "both users' rules must be swept into AGE");

    // Sanity check at the AGE layer: both titles really are there,
    // confirming the leak is reachable if the service doesn't filter it.
    let vertices = fubbik_db::age::list_behavior_rule_vertices(&pool)
        .await
        .unwrap();
    let all_titles: Vec<&str> = vertices.iter().map(|v| v.title.as_str()).collect();
    assert!(all_titles.contains(&"A's secret rule"));
    assert!(all_titles.contains(&"B's secret rule"));
    let all_edges = fubbik_db::age::list_governs_edges(&pool).await.unwrap();
    assert!(
        all_edges.iter().any(|e| e.source_id == rule_b_id),
        "B's governs edge must exist in AGE, or the edge-filter assertion \
         below would pass vacuously"
    );

    let res = send(
        app.clone(),
        &cookie_a,
        "GET",
        "/api/graph",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let returned_titles: Vec<&str> = body["behaviorRules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["title"].as_str().unwrap())
        .collect();

    assert!(
        returned_titles.contains(&"A's secret rule"),
        "user A's own rule must still be returned"
    );
    assert!(
        !returned_titles.contains(&"B's secret rule"),
        "user A's graph must not contain user B's behavior rule title, got {returned_titles:?}"
    );

    let returned_edges = body["governsEdges"].as_array().unwrap();
    assert!(
        !returned_edges
            .iter()
            .any(|e| e["sourceId"].as_str() == Some(rule_b_id.as_str())),
        "user A's graph must not contain a governs edge for user B's rule, got {returned_edges:?}"
    );
}

/// `service::build`'s two `unwrap_or_default()` calls are the only thing
/// keeping AGE-side failures off `GET /api/graph` as a 500. Every other test
/// in this suite runs against an intact `"knowledge"` graph, so none of them
/// exercise that path. This one drops the graph itself (the "knowledge"
/// graph created for THIS TEST's own ephemeral `#[sqlx::test]` database by
/// migration `0001_init.sql` — not any shared database) so the extension is
/// still present (`age::is_available` stays true) but every Cypher call
/// against it fails, and asserts the endpoint still returns 200 with empty
/// arrays rather than an error.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn graph_degrades_to_empty_behavior_fields_when_the_graph_is_missing(pool: sqlx::PgPool) {
    use sqlx::{Acquire, Executor};

    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }

    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    // A chunk (plain SQL, unaffected by AGE) so we can also assert the rest
    // of the payload still comes back populated — this must be a targeted
    // degradation, not the whole endpoint going empty.
    let chunk = send(
        app.clone(),
        &cookie,
        "POST",
        "/api/chunks",
        serde_json::json!({ "title": "Survives", "content": "x", "type": "note" }),
    )
    .await;
    assert_eq!(chunk.status(), StatusCode::CREATED);

    {
        let mut conn = pool.acquire().await.unwrap();
        conn.execute("LOAD 'age';").await.unwrap();
        let mut tx = conn.begin().await.unwrap();
        sqlx::query(r#"SET LOCAL search_path = ag_catalog, "$user", public;"#)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("SELECT drop_graph('knowledge', true);")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }

    // The extension itself is still installed — only the graph is gone.
    assert!(fubbik_db::age::is_available(&pool).await);

    let res = send(app, &cookie, "GET", "/api/graph", serde_json::Value::Null).await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "a missing graph must degrade, not 500"
    );
    let body = json_body(res).await;

    assert_eq!(
        body["chunks"].as_array().unwrap().len(),
        1,
        "the non-AGE half of the payload must be unaffected"
    );
    assert!(body["behaviorRules"].as_array().unwrap().is_empty());
    assert!(body["governsEdges"].as_array().unwrap().is_empty());
}
