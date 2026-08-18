//! HTTP-level tests for the `proposals` domain
//! (`packages/api/src/proposals/`).
//!
//! **Ownership guard-removal proof for `approve` lives here, not in
//! `fubbik-db`'s test suite.** `approve_proposal` enforces chunk ownership
//! only by delegating to `chunks::service::update`, whose `chunk::update`
//! repository call carries `WHERE id = .. AND user_id = ..` in its own SQL
//! — there is no separate pre-check anywhere in the proposals service layer
//! that could mask a removed guard, so an API-level request is exactly
//! where a regression there would first go red (see
//! `cross_user_approve_is_404_and_leaves_both_chunk_and_proposal_unchanged`).
//! `crates/fubbik-db/tests/proposal.rs` documents why the equivalent DB-only
//! proof isn't meaningful there: this repository's own functions carry no
//! `user_id` parameter at all.
//!
//! **`reject` has no such guard, and that is Node's actual behavior, not a
//! gap this port introduces** — see
//! `cross_user_reject_succeeds_because_node_has_no_ownership_check`.

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
                .body(Body::from(
                    serde_json::json!({ "title": title, "content": "original" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await["id"].as_str().unwrap().to_string()
}

async fn get_chunk(app: axum::Router, cookie: &str, id: &str) -> serde_json::Value {
    let res = app
        .oneshot(
            Request::get(format!("/api/chunks/{id}"))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await
}

async fn create_proposal(
    app: axum::Router,
    cookie: &str,
    chunk_id: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post(format!("/api/chunks/{chunk_id}/proposals"))
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn create_pending_proposal(
    app: axum::Router,
    cookie: &str,
    chunk_id: &str,
    title: &str,
) -> String {
    let res = create_proposal(
        app,
        cookie,
        chunk_id,
        serde_json::json!({ "changes": { "title": title } }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await["id"].as_str().unwrap().to_string()
}

async fn approve(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::post(format!("/api/proposals/{id}/approve"))
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from("{}"))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn reject(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::post(format!("/api/proposals/{id}/reject"))
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from("{}"))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_proposal(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/proposals/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_rejects_empty_changes(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-empty@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;

    let res = create_proposal(
        app,
        &cookie,
        &chunk_id,
        serde_json::json!({ "changes": {} }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    // `AppError::Validation`'s `Display` prepends "validation failed: " to
    // the message — see `crates/fubbik-api/tests/tags.rs` for the same note.
    assert_eq!(
        body["message"],
        "validation failed: changes must not be empty"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_has_no_ownership_check_any_authenticated_user_may_propose(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-propose@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-propose@b.test", "Bob").await;
    let alice_chunk = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;

    // Bob, who does not own the chunk, proposes changes to it. Node's
    // createProposal never checks this — see fubbik_db::repo::proposal's
    // module doc comment.
    let res = create_proposal(
        app.clone(),
        &bob_cookie,
        &alice_chunk,
        serde_json::json!({ "changes": { "title": "Bob's suggestion" } }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "Node places no ownership check on proposal creation"
    );
    let body = json_body(res).await;
    assert_eq!(body["changes"]["title"], "Bob's suggestion");
    assert_eq!(body["status"], "pending");
    assert!(body["proposedBy"].is_string());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_chunk_proposals_orders_ascending_and_accepts_any_status(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;

    let first = create_pending_proposal(app.clone(), &cookie, &chunk_id, "v1").await;
    let second = create_pending_proposal(app.clone(), &cookie, &chunk_id, "v2").await;

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/chunks/{chunk_id}/proposals"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let ids: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![first.as_str(), second.as_str()],
        "must be ascending by createdAt, the opposite direction from the global queue"
    );

    // status is unvalidated free text at this endpoint — a nonsense value
    // matches zero rows rather than erroring.
    let res = app
        .oneshot(
            Request::get(format!(
                "/api/chunks/{chunk_id}/proposals?status=totally-bogus"
            ))
            .header("cookie", &cookie)
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(body.as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn global_list_defaults_to_pending_and_validates_status(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-global@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;
    let proposal_id = create_pending_proposal(app.clone(), &cookie, &chunk_id, "v1").await;

    // No ?status= at all -> defaults to pending, not "every status".
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/proposals")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let ids: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![proposal_id.as_str()]);
    assert_eq!(body[0]["chunkTitle"], "Chunk");
    assert_eq!(body[0]["chunkType"], "note");

    // Approve it, then the default (pending) view no longer shows it.
    approve(app.clone(), &cookie, &proposal_id).await;
    let body = json_body(
        app.clone()
            .oneshot(
                Request::get("/api/proposals")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert!(body.as_array().unwrap().is_empty());

    // An explicit bad status is a 400, unlike the chunk-scoped endpoint.
    let res = app
        .oneshot(
            Request::get("/api/proposals?status=bogus")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn count_returns_pending_object_shape(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-count@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/proposals/count")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({ "pending": 0 }),
        "stats-bar.tsx reads .pending off this response, not .count or a bare number"
    );

    create_pending_proposal(app.clone(), &cookie, &chunk_id, "v1").await;
    create_pending_proposal(app.clone(), &cookie, &chunk_id, "v2").await;

    let body = json_body(
        app.oneshot(
            Request::get("/api/proposals/count")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(body, serde_json::json!({ "pending": 2 }));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn approve_applies_changes_to_the_chunk_and_marks_the_proposal_approved(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-approve@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Original title").await;
    let proposal_id = create_pending_proposal(app.clone(), &cookie, &chunk_id, "New title").await;

    let res = approve(app.clone(), &cookie, &proposal_id).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["status"], "approved");
    assert!(body["reviewedAt"].is_string());

    let chunk = get_chunk(app.clone(), &cookie, &chunk_id).await;
    assert_eq!(
        chunk["title"], "New title",
        "approving must write the proposed change through to the chunk"
    );

    // Re-fetching the proposal directly also reflects the approval.
    let refetched = json_body(get_proposal(app, &cookie, &proposal_id).await).await;
    assert_eq!(refetched["status"], "approved");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn approving_an_already_reviewed_proposal_is_400(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-double@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;
    let proposal_id = create_pending_proposal(app.clone(), &cookie, &chunk_id, "v1").await;

    assert_eq!(
        approve(app.clone(), &cookie, &proposal_id).await.status(),
        StatusCode::OK
    );

    let res = approve(app.clone(), &cookie, &proposal_id).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    assert_eq!(
        body["message"],
        "validation failed: Proposal is already approved"
    );
}

/// The load-bearing guard-removal proof for `approve`'s ownership check —
/// see this file's module doc comment for why the API layer, not
/// `fubbik-db`, is where this must be proven. Bob (not the chunk's owner)
/// attempts to approve a proposal against Alice's chunk: this must 404, the
/// chunk's title must be untouched, AND the proposal must remain `pending`
/// — proving the two-write, non-atomic order (chunk write attempted and
/// rejected by its own ownership check *before* the proposal row is ever
/// touched), not just that the HTTP call failed.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_approve_is_404_and_leaves_both_chunk_and_proposal_unchanged(
    pool: sqlx::PgPool,
) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-crossapprove@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-crossapprove@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's original title").await;
    let proposal_id = create_pending_proposal(
        app.clone(),
        &alice_cookie,
        &chunk_id,
        "Bob's proposed title",
    )
    .await;

    let res = approve(app.clone(), &bob_cookie, &proposal_id).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "a non-owner must not be able to approve a proposal against someone else's chunk"
    );

    let chunk = get_chunk(app.clone(), &alice_cookie, &chunk_id).await;
    assert_eq!(
        chunk["title"], "Alice's original title",
        "the chunk must be untouched by Bob's rejected approve"
    );

    let proposal = json_body(get_proposal(app, &alice_cookie, &proposal_id).await).await;
    assert_eq!(
        proposal["status"], "pending",
        "the proposal must remain pending — the ownership check fails before \
         the chunk write, so the proposal-status write is never reached either"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn reject_marks_the_proposal_rejected_without_touching_the_chunk(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-reject@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Original title").await;
    let proposal_id =
        create_pending_proposal(app.clone(), &cookie, &chunk_id, "Proposed title").await;

    let res = reject(app.clone(), &cookie, &proposal_id).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["status"], "rejected");

    let chunk = get_chunk(app, &cookie, &chunk_id).await;
    assert_eq!(
        chunk["title"], "Original title",
        "reject must never apply the proposed changes to the chunk"
    );
}

/// Documents Node's actual (asymmetric, arguably surprising) behavior: does
/// **not** wrap it in a check this port doesn't have — `rejectProposal`
/// never calls `updateChunk`, so it never derives chunk ownership at all.
/// Bob, who does not own Alice's chunk, can reject a proposal against it.
/// See `fubbik_db::repo::proposal`'s module doc comment; this is flagged in
/// the phase report as a concern for a human decision, not silently
/// "fixed" here.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_reject_succeeds_because_node_has_no_ownership_check(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-crossreject@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-crossreject@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;
    let proposal_id =
        create_pending_proposal(app.clone(), &alice_cookie, &chunk_id, "Some change").await;

    let res = reject(app, &bob_cookie, &proposal_id).await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "faithfully reproducing Node: reject has no chunk-ownership check at all"
    );
    let body = json_body(res).await;
    assert_eq!(body["status"], "rejected");
    assert!(
        body["reviewedBy"].is_string(),
        "reviewedBy must be set to Bob's user id, proving his (unauthorized-by-ownership) \
         reject actually went through"
    );
}

/// Bulk actions are sequential and fail-fast: the second entry references a
/// nonexistent proposal, so the whole request 404s — but the first entry's
/// write (approving `p1`, which mutates both the chunk and the proposal
/// row) has already committed and is NOT rolled back. Matches Node's
/// `Effect.forEach(..., { concurrency: 1 })`, which has no transaction
/// around the loop.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn bulk_action_is_sequential_and_fail_fast_without_rolling_back_earlier_writes(
    pool: sqlx::PgPool,
) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-bulk@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Original").await;
    let p1 = create_pending_proposal(app.clone(), &cookie, &chunk_id, "Approved via bulk").await;

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/proposals/bulk")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::json!({
                        "actions": [
                            { "proposalId": p1, "action": "approve" },
                            { "proposalId": "no-such-proposal", "action": "approve" },
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // p1's approval already committed despite the overall request failing.
    let chunk = get_chunk(app.clone(), &cookie, &chunk_id).await;
    assert_eq!(chunk["title"], "Approved via bulk");
    let proposal = json_body(get_proposal(app, &cookie, &p1).await).await;
    assert_eq!(proposal["status"], "approved");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn bulk_action_approves_and_rejects_in_one_call(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-bulk2@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;
    let p1 = create_pending_proposal(app.clone(), &cookie, &chunk_id, "Approve me").await;
    let p2 = create_pending_proposal(app.clone(), &cookie, &chunk_id, "Reject me").await;

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/proposals/bulk")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::json!({
                        "actions": [
                            { "proposalId": p1, "action": "approve" },
                            { "proposalId": p2, "action": "reject", "note": "not needed" },
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body[0]["status"], "approved");
    assert_eq!(body[1]["status"], "rejected");
    assert_eq!(body[1]["reviewNote"], "not needed");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn not_found_message_is_titlecase(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-404@b.test", "Alice").await;

    let res = get_proposal(app.clone(), &cookie, "no-such-id").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_body(res).await["message"], "Proposal not found");

    let res = approve(app.clone(), &cookie, "no-such-id").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_body(res).await["message"], "Proposal not found");

    let res = reject(app, &cookie, "no-such-id").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_body(res).await["message"], "Proposal not found");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_requests_are_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .clone()
        .oneshot(Request::get("/api/proposals").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = app
        .oneshot(
            Request::get("/api/proposals/count")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
