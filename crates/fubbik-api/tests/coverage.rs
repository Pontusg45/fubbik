//! HTTP-level tests for the `coverage` domain:
//! `GET /api/requirements/coverage` and `GET /api/requirements/traceability`.
//!
//! Two things about this endpoint pair are easy to get wrong and are pinned
//! here rather than at the repo layer, because both are decided above the
//! SQL:
//!
//! 1. **`/requirements/coverage` has two response shapes.** `?detail=true`
//!    adds a `matrix` key; every other value of `detail` — including
//!    `?detail=1`, which reads like a boolean and is not one — omits the key
//!    entirely. Node dispatches on `ctx.query.detail === "true"`
//!    (`packages/api/src/coverage/routes.ts:14`), a literal string compare.
//!    A test asserting only "matrix is empty on the default path" would pass
//!    against an implementation that always computed and returned it, which
//!    would silently double the query count of the common request.
//!
//! 2. **The space filter is spelled `codebaseId`, not `spaceId`.** That's the
//!    deprecated name, still on the wire — see `coverage::dto::CoverageQuery`.
//!    The test below sends `spaceId` and asserts it is *ignored*, because a
//!    port that "helpfully" renamed the parameter would still pass a test
//!    that only ever sent `codebaseId`.
//!
//! Cross-user guard-removal proofs live at the repo layer
//! (`crates/fubbik-db/tests/coverage.rs`), per this codebase's convention.
//! The user-scoping test here is the end-to-end counterpart, not the proof.

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

async fn create_chunk(app: axum::Router, cookie: &str, title: &str) -> String {
    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(serde_json::json!({"title": title}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "chunk creation must succeed");
    json_body(res).await["id"].as_str().unwrap().to_string()
}

async fn create_requirement(
    app: axum::Router,
    cookie: &str,
    title: &str,
    space_id: Option<&str>,
) -> String {
    let mut body = serde_json::json!({
        "title": title,
        "steps": [
            {"keyword": "given", "text": "a user"},
            {"keyword": "when", "text": "they log in"},
            {"keyword": "then", "text": "they see the dashboard"},
        ],
    });
    if let Some(s) = space_id {
        body["spaceId"] = serde_json::Value::String(s.to_string());
    }
    let res = app
        .oneshot(
            Request::post("/api/requirements")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "requirement creation must succeed"
    );
    json_body(res).await["requirement"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_space(app: axum::Router, cookie: &str, name: &str) -> String {
    let res = app
        .oneshot(
            Request::post("/api/spaces")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(
                    serde_json::json!({"name": name, "kind": "code"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "space creation must succeed, got {}",
        res.status()
    );
    json_body(res).await["id"].as_str().unwrap().to_string()
}

async fn link_chunks(app: axum::Router, cookie: &str, requirement_id: &str, chunk_ids: &[&str]) {
    let res = app
        .oneshot(
            Request::put(format!("/api/requirements/{requirement_id}/chunks"))
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(
                    serde_json::json!({"chunkIds": chunk_ids}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "chunk linking must succeed");
}

async fn get(app: axum::Router, cookie: &str, uri: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(uri)
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn coverage(app: axum::Router, cookie: &str, query: &str) -> serde_json::Value {
    let res = get(app, cookie, &format!("/api/requirements/coverage{query}")).await;
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await
}

/// The `covered`/`uncovered` arrays have no defined order (no `ORDER BY` in
/// Node or in the port — see `fubbik_db::repo::coverage`'s module doc), so
/// assertions go through this rather than indexing positionally.
fn sorted_titles(value: &serde_json::Value) -> Vec<String> {
    let mut v: Vec<String> = value
        .as_array()
        .expect("expected an array")
        .iter()
        .map(|c| c["title"].as_str().unwrap().to_string())
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------
// GET /api/requirements/coverage — the two shapes
// ---------------------------------------------------------------------

/// Without `detail`, the `matrix` key must be **absent**, not `null` and not
/// `[]`. Node builds the default response from `getCoverage`, which never
/// mentions `matrix` (`packages/api/src/coverage/service.ts:40-44`).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn default_response_omits_the_matrix_key_entirely(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cov-default@b.test", "Alice").await;

    let c = create_chunk(app.clone(), &cookie, "Auth").await;
    let r = create_requirement(app.clone(), &cookie, "Login works", None).await;
    link_chunks(app.clone(), &cookie, &r, &[&c]).await;

    let body = coverage(app.clone(), &cookie, "").await;
    let obj = body.as_object().unwrap();
    assert!(
        !obj.contains_key("matrix"),
        "default response must not carry a matrix key at all, got {body}"
    );
    // `serde_json::Map` is a `BTreeMap` here, so key order is alphabetical
    // and says nothing about the struct — only the key *set* is asserted.
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["covered", "stats", "uncovered"],
        "exactly Node's three keys"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detail_true_adds_the_matrix_key(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cov-detail@b.test", "Alice").await;

    let c = create_chunk(app.clone(), &cookie, "Auth").await;
    create_chunk(app.clone(), &cookie, "Orphan").await;
    let r = create_requirement(app.clone(), &cookie, "Login works", None).await;
    link_chunks(app.clone(), &cookie, &r, &[&c]).await;

    let body = coverage(app.clone(), &cookie, "?detail=true").await;
    let matrix = body["matrix"].as_array().expect("matrix must be present");
    assert_eq!(matrix.len(), 1, "one (chunk, requirement) pair");
    assert_eq!(matrix[0]["chunkId"].as_str().unwrap(), c);
    assert_eq!(matrix[0]["chunkTitle"].as_str().unwrap(), "Auth");
    assert_eq!(matrix[0]["requirementId"].as_str().unwrap(), r);
    assert_eq!(
        matrix[0]["requirementTitle"].as_str().unwrap(),
        "Login works"
    );
    assert_eq!(matrix[0]["requirementStatus"].as_str().unwrap(), "untested");

    assert_eq!(
        body["covered"].as_array().unwrap().len(),
        1,
        "detail=true still carries the plain coverage fields"
    );
    assert_eq!(body["uncovered"].as_array().unwrap().len(), 1);
}

/// `detail` is compared with the literal string `"true"`, so every other
/// truthy-looking value takes the default branch. This is the test that
/// fails if `detail` is modelled as `Option<bool>`: serde would accept
/// `?detail=1` and add the matrix, which Node does not.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn only_the_literal_string_true_selects_the_detail_shape(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cov-literal@b.test", "Alice").await;
    create_chunk(app.clone(), &cookie, "Auth").await;

    for q in [
        "?detail=1",
        "?detail=TRUE",
        "?detail=True",
        "?detail=yes",
        "?detail=",
    ] {
        let body = coverage(app.clone(), &cookie, q).await;
        assert!(
            !body.as_object().unwrap().contains_key("matrix"),
            "`{q}` must take the default branch, got {body}"
        );
    }

    let body = coverage(app.clone(), &cookie, "?detail=true").await;
    assert!(body.as_object().unwrap().contains_key("matrix"));
}

// ---------------------------------------------------------------------
// stats arithmetic
// ---------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn stats_partition_and_percentage(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cov-stats@b.test", "Alice").await;

    let a = create_chunk(app.clone(), &cookie, "A").await;
    let b = create_chunk(app.clone(), &cookie, "B").await;
    create_chunk(app.clone(), &cookie, "C").await;
    create_chunk(app.clone(), &cookie, "D").await;

    let r1 = create_requirement(app.clone(), &cookie, "R1", None).await;
    let r2 = create_requirement(app.clone(), &cookie, "R2", None).await;
    link_chunks(app.clone(), &cookie, &r1, &[&a, &b]).await;
    link_chunks(app.clone(), &cookie, &r2, &[&a]).await;

    let body = coverage(app.clone(), &cookie, "").await;
    assert_eq!(sorted_titles(&body["covered"]), vec!["A", "B"]);
    assert_eq!(sorted_titles(&body["uncovered"]), vec!["C", "D"]);
    assert_eq!(
        body["stats"],
        serde_json::json!({"total": 4, "covered": 2, "uncovered": 2, "percentage": 50})
    );

    let counts: std::collections::BTreeMap<String, i64> = body["covered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["title"].as_str().unwrap().to_string(),
                c["requirementCount"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(counts["A"], 2, "A is linked from both requirements");
    assert_eq!(counts["B"], 1);
    assert!(
        body["uncovered"].as_array().unwrap()[0]
            .as_object()
            .unwrap()
            .get("requirementCount")
            .is_none(),
        "uncovered entries are {{id, title}} only — no requirementCount"
    );
}

/// The zero-total case. In JS `0 / 0` is `NaN` and `JSON.stringify(NaN)` is
/// `null`, which is why Node writes `total > 0 ? ... : 0`
/// (`packages/api/src/coverage/service.ts:38`) — without that guard the
/// field would come back `null` and the web client's percentage bar would
/// render `NaN%`.
///
/// This test pins the *observable* contract (`percentage: 0`, and both
/// arrays present and empty rather than absent). It deliberately does not
/// claim to prove the Rust guard load-bearing: deleting `total > 0` from
/// `service::get_coverage` leaves this green, because Rust's float-to-int
/// cast saturates and `NaN as i64` is `0`. That was measured, not assumed —
/// see the guard's doc comment.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn percentage_is_zero_when_there_are_no_chunks(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cov-empty@b.test", "Alice").await;

    let body = coverage(app.clone(), &cookie, "").await;
    assert_eq!(
        body["stats"],
        serde_json::json!({"total": 0, "covered": 0, "uncovered": 0, "percentage": 0})
    );
    assert_eq!(body["covered"], serde_json::json!([]));
    assert_eq!(body["uncovered"], serde_json::json!([]));
}

/// `Math.round` on `1/3`: 33.33… -> 33, and on `2/3`: 66.67… -> 67. Pins
/// that rounding is half-away-from-zero rounding of the *percentage*, not
/// truncation and not rounding of the ratio.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn percentage_rounds_to_the_nearest_whole_number(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cov-round@b.test", "Alice").await;

    let a = create_chunk(app.clone(), &cookie, "A").await;
    create_chunk(app.clone(), &cookie, "B").await;
    create_chunk(app.clone(), &cookie, "C").await;
    let r = create_requirement(app.clone(), &cookie, "R", None).await;
    link_chunks(app.clone(), &cookie, &r, &[&a]).await;

    let body = coverage(app.clone(), &cookie, "").await;
    assert_eq!(body["stats"]["percentage"], 33, "1/3 rounds down");

    let b = create_chunk(app.clone(), &cookie, "D").await;
    let c = create_chunk(app.clone(), &cookie, "E").await;
    let r2 = create_requirement(app.clone(), &cookie, "R2", None).await;
    link_chunks(app.clone(), &cookie, &r2, &[&b, &c]).await;

    // 3 covered of 5 -> 60%.
    let body = coverage(app.clone(), &cookie, "").await;
    assert_eq!(
        body["stats"],
        serde_json::json!({
            "total": 5, "covered": 3, "uncovered": 2, "percentage": 60
        })
    );
}

// ---------------------------------------------------------------------
// codebaseId (the deprecated spelling, still the wire contract)
// ---------------------------------------------------------------------

/// Sends `codebaseId` and asserts it filters; then sends `spaceId` and
/// asserts it is *ignored*. Without the second half, a port that renamed
/// the parameter to `spaceId` would pass.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn filters_on_codebase_id_and_ignores_space_id(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-cov-space@b.test", "Alice").await;

    let backend = create_space(app.clone(), &cookie, "backend").await;
    let in_space = create_chunk(app.clone(), &cookie, "API notes").await;
    create_chunk(app.clone(), &cookie, "Global notes").await;
    sqlx::query!(
        "INSERT INTO chunk_space (chunk_id, space_id) VALUES ($1, $2)",
        in_space,
        backend
    )
    .execute(&pool)
    .await
    .unwrap();

    let filtered = coverage(app.clone(), &cookie, &format!("?codebaseId={backend}")).await;
    assert_eq!(sorted_titles(&filtered["uncovered"]), vec!["API notes"]);
    assert_eq!(filtered["stats"]["total"], 1);

    let by_wrong_name = coverage(app.clone(), &cookie, &format!("?spaceId={backend}")).await;
    assert_eq!(
        by_wrong_name["stats"]["total"], 2,
        "`spaceId` is not this endpoint's parameter — it must be ignored, \
         leaving the response unfiltered"
    );
}

/// `?codebaseId=` (blank) must behave as *absent*, because Node reaches the
/// filter through `if (codebaseId)` and `""` is falsy in JS. Without the
/// `blank_to_none` normalisation this filters on `space_id = ''` and returns
/// nothing.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn blank_codebase_id_means_unfiltered(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cov-blank@b.test", "Alice").await;
    create_chunk(app.clone(), &cookie, "A").await;
    create_chunk(app.clone(), &cookie, "B").await;

    let body = coverage(app.clone(), &cookie, "?codebaseId=").await;
    assert_eq!(body["stats"]["total"], 2);

    let detailed = coverage(app.clone(), &cookie, "?codebaseId=&detail=true").await;
    assert_eq!(detailed["stats"]["total"], 2);
}

// ---------------------------------------------------------------------
// user scoping, end to end
// ---------------------------------------------------------------------

/// End-to-end counterpart of the repo-layer guard-removal proofs. There is
/// no service-level pre-check on this path, so this genuinely exercises the
/// SQL predicates — but the proof that they are load-bearing is in
/// `crates/fubbik-db/tests/coverage.rs`, where the predicate can be removed
/// in isolation.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn coverage_never_reports_another_users_chunks(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "alice-cov-scope@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-cov-scope@b.test", "Bob").await;

    let ac = create_chunk(app.clone(), &alice, "Alice secret").await;
    let ar = create_requirement(app.clone(), &alice, "Alice requirement", None).await;
    link_chunks(app.clone(), &alice, &ar, &[&ac]).await;
    create_chunk(app.clone(), &bob, "Bob chunk").await;

    let bobs = coverage(app.clone(), &bob, "?detail=true").await;
    assert_eq!(sorted_titles(&bobs["uncovered"]), vec!["Bob chunk"]);
    assert_eq!(bobs["covered"], serde_json::json!([]));
    assert_eq!(
        bobs["matrix"],
        serde_json::json!([]),
        "the matrix must not leak another user's chunk/requirement titles"
    );
    assert_eq!(bobs["stats"]["total"], 1);

    let alices = coverage(app.clone(), &alice, "?detail=true").await;
    assert_eq!(sorted_titles(&alices["covered"]), vec!["Alice secret"]);
    assert_eq!(alices["matrix"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn both_endpoints_require_a_session(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    for uri in [
        "/api/requirements/coverage",
        "/api/requirements/traceability",
    ] {
        let res = app
            .clone()
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "{uri} must require a session"
        );
    }
}

/// Both paths are static segments that sit under `requirements`' `/{id}`
/// route. If axum ever preferred the parameterised route, these requests
/// would 404 (no requirement with id `coverage`) instead of 200.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn static_paths_win_over_the_requirement_id_route(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-cov-routing@b.test", "Alice").await;

    for uri in [
        "/api/requirements/coverage",
        "/api/requirements/traceability",
    ] {
        let res = get(app.clone(), &cookie, uri).await;
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "{uri} must not fall through to /{{id}}"
        );
    }
}

// ---------------------------------------------------------------------
// GET /api/requirements/traceability
// ---------------------------------------------------------------------

/// A bare array, and every row carries empty `planSteps`/`sessions`. Those
/// two are hard-coded empty in Node under a `TODO` — see
/// `coverage::dto::TraceabilityRow`. The web client reads `.length` on both,
/// so "absent" and "empty" are not interchangeable.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn traceability_returns_a_bare_array_with_empty_plan_steps_and_sessions(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-trace@b.test", "Alice").await;

    let c = create_chunk(app.clone(), &cookie, "Auth").await;
    let r = create_requirement(app.clone(), &cookie, "Login works", None).await;
    link_chunks(app.clone(), &cookie, &r, &[&c]).await;

    let res = get(app.clone(), &cookie, "/api/requirements/traceability").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;

    let rows = body.as_array().expect("traceability returns a bare array");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0],
        serde_json::json!({
            "id": r,
            "title": "Login works",
            "status": "untested",
            "priority": null,
            "planSteps": [],
            "sessions": [],
        }),
        "exactly Node's six keys, with both arrays present and empty"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn traceability_filters_on_codebase_id(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-trace-space@b.test", "Alice").await;

    let backend = create_space(app.clone(), &cookie, "backend").await;
    create_requirement(app.clone(), &cookie, "Backend req", Some(&backend)).await;
    create_requirement(app.clone(), &cookie, "Global req", None).await;

    let res = get(
        app.clone(),
        &cookie,
        &format!("/api/requirements/traceability?codebaseId={backend}"),
    )
    .await;
    let body = json_body(res).await;
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["title"], "Backend req");

    let all = get(app.clone(), &cookie, "/api/requirements/traceability").await;
    assert_eq!(json_body(all).await.as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn traceability_never_reports_another_users_requirements(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "alice-trace-scope@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-trace-scope@b.test", "Bob").await;

    create_requirement(app.clone(), &alice, "Alice requirement", None).await;
    create_requirement(app.clone(), &bob, "Bob requirement", None).await;

    let res = get(app.clone(), &bob, "/api/requirements/traceability").await;
    let body = json_body(res).await;
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["title"], "Bob requirement");
}
