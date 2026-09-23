//! HTTP-level tests for the `proposals` domain
//! (`packages/api/src/proposals/`).
//!
//! **The mandatory guard-removal proofs (Phase 2e wave 1: `list`/`get`
//! scoping and `reject`'s ownership check) live in
//! `crates/fubbik-db/tests/proposal.rs`, at the repository layer** — per the
//! task brief, an API-level request alone can't prove a SQL-level guard is
//! load-bearing when nothing else in the call path would mask its removal.
//! `approve`'s cross-user case is the one exception still proven here too:
//! its ownership check has always lived on the chunk `UPDATE` itself, with
//! no service-level pre-check anywhere upstream that could mask a
//! regression, so an API-level request is a meaningful (if not sufficient
//! on its own) additional proof — see
//! `cross_user_approve_is_404_and_leaves_both_chunk_and_proposal_unchanged`.
//! The tests below for `list`/`get`/`reject` are end-to-end confirmations of
//! the same behaviour, not substitutes for the repo-level proofs.

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
    assert_eq!(res.status(), StatusCode::CREATED);
    json_body(res).await["id"].as_str().unwrap().to_string()
}

/// Returns just the chunk row from `GET /api/chunks/{id}`.
///
/// That route answers with the enriched detail envelope
/// (`chunks::dto::ChunkDetail`), so the row lives under `chunk`. This file
/// only ever asserts on the row's own fields, so the helper unwraps it
/// rather than making every call site say `["chunk"]`. The envelope itself
/// is covered by `tests/chunk_detail.rs`.
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
    json_body(res).await["chunk"].clone()
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
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-empty@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;

    // When
    let res = create_proposal(
        app,
        &cookie,
        &chunk_id,
        serde_json::json!({ "changes": {} }),
    )
    .await;
    // Then
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
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-propose@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-propose@b.test", "Bob").await;
    let alice_chunk = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;

    // When
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
    // Then
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
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;

    let first = create_pending_proposal(app.clone(), &cookie, &chunk_id, "v1").await;
    let second = create_pending_proposal(app.clone(), &cookie, &chunk_id, "v2").await;

    // When
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
    // Then
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
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-global@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;
    let proposal_id = create_pending_proposal(app.clone(), &cookie, &chunk_id, "v1").await;

    // When
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
    // Then
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

/// End-to-end confirmation of Phase 2e wave 1's `list` scoping — the
/// mandatory SQL-level proof lives in
/// `fubbik-db/tests/proposal.rs::list_is_scoped_to_the_caller`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn global_list_does_not_leak_another_users_proposals(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-list-scope@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-list-scope@b.test", "Bob").await;
    let alice_chunk = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;
    let bob_chunk = create_chunk(app.clone(), &bob_cookie, "Bob's chunk").await;
    create_pending_proposal(app.clone(), &alice_cookie, &alice_chunk, "a-change").await;
    create_pending_proposal(app.clone(), &bob_cookie, &bob_chunk, "b-change").await;

    // When
    let alice_view = json_body(
        app.clone()
            .oneshot(
                Request::get("/api/proposals")
                    .header("cookie", &alice_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let alice_chunks: Vec<&str> = alice_view
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["chunkId"].as_str().unwrap())
        .collect();
    // Then
    assert_eq!(
        alice_chunks,
        vec![alice_chunk.as_str()],
        "Alice must not see Bob's proposal in the global queue"
    );

    let bob_view = json_body(
        app.oneshot(
            Request::get("/api/proposals")
                .header("cookie", &bob_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    let bob_chunks: Vec<&str> = bob_view
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["chunkId"].as_str().unwrap())
        .collect();
    assert_eq!(bob_chunks, vec![bob_chunk.as_str()]);
}

/// End-to-end confirmation of Phase 2e wave 1's `get` scoping — the
/// mandatory SQL-level proof lives in
/// `fubbik-db/tests/proposal.rs::find_by_id_for_owner_is_scoped`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_by_id_is_404_for_a_proposal_on_another_users_chunk(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-get-scope@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-get-scope@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;
    let proposal_id = create_pending_proposal(app.clone(), &alice_cookie, &chunk_id, "v2").await;

    // When
    let res = get_proposal(app.clone(), &bob_cookie, &proposal_id).await;
    // Then
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "Bob must not be able to read Alice's proposal by id"
    );

    let res = get_proposal(app, &alice_cookie, &proposal_id).await;
    assert_eq!(res.status(), StatusCode::OK, "Alice can read her own");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn count_returns_pending_object_shape(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-count@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;

    // When
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
    // Then
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
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-approve@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Original title").await;
    let proposal_id = create_pending_proposal(app.clone(), &cookie, &chunk_id, "New title").await;

    // When
    let res = approve(app.clone(), &cookie, &proposal_id).await;
    // Then
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

/// End-to-end confirmation of Fix 1 (the data-loss bug): `alternatives` and
/// `scope` used to be silently dropped on approve. The mandatory proof,
/// including `tags` (which has no `GET /chunks/{id}` field to check here —
/// `chunk_tag` is a join table), lives in
/// `fubbik-db/tests/proposal.rs::approve_applies_every_proposed_changes_field`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn approve_no_longer_drops_alternatives_and_scope(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-fields@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Original title").await;

    // When
    let res = create_proposal(
        app.clone(),
        &cookie,
        &chunk_id,
        serde_json::json!({
            "changes": {
                "alternatives": ["do nothing", "wait and see"],
                "scope": { "area": "backend" }
            }
        }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let proposal_id = json_body(res).await["id"].as_str().unwrap().to_string();

    let res = approve(app.clone(), &cookie, &proposal_id).await;
    assert_eq!(res.status(), StatusCode::OK);

    let chunk = get_chunk(app, &cookie, &chunk_id).await;
    assert_eq!(
        chunk["alternatives"],
        serde_json::json!(["do nothing", "wait and see"]),
        "alternatives must land on the chunk, not be silently dropped"
    );
    assert_eq!(
        chunk["scope"],
        serde_json::json!({ "area": "backend" }),
        "scope must land on the chunk, not be silently dropped"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn approving_an_already_reviewed_proposal_is_400(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-double@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;
    // When
    let proposal_id = create_pending_proposal(app.clone(), &cookie, &chunk_id, "v1").await;

    // Then
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
    // Given
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

    // When
    let res = approve(app.clone(), &bob_cookie, &proposal_id).await;
    // Then
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
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-reject@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Original title").await;
    let proposal_id =
        create_pending_proposal(app.clone(), &cookie, &chunk_id, "Proposed title").await;

    // When
    let res = reject(app.clone(), &cookie, &proposal_id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["status"], "rejected");

    let chunk = get_chunk(app, &cookie, &chunk_id).await;
    assert_eq!(
        chunk["title"], "Original title",
        "reject must never apply the proposed changes to the chunk"
    );
}

/// Phase 2e wave 1: `reject` is now scoped through the parent chunk, the
/// same as `approve` — Node itself has no such check
/// (`rejectProposal` never calls `updateChunk`), but this port now
/// deliberately diverges. Bob, who does not own Alice's chunk, must be
/// rejected with 404, and the proposal must remain `pending` — the SQL-level
/// guard-removal proof for this lives in
/// `fubbik-db/tests/proposal.rs::reject_is_scoped_through_the_parent_chunk`;
/// this is the end-to-end confirmation.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_reject_is_404_and_leaves_the_proposal_pending(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-crossreject@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-crossreject@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;
    let proposal_id =
        create_pending_proposal(app.clone(), &alice_cookie, &chunk_id, "Some change").await;

    // When
    let res = reject(app.clone(), &bob_cookie, &proposal_id).await;
    // Then
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "a non-owner must not be able to reject a proposal against someone else's chunk"
    );

    let proposal = json_body(get_proposal(app, &alice_cookie, &proposal_id).await).await;
    assert_eq!(
        proposal["status"], "pending",
        "Bob's rejected reject attempt must leave the proposal pending"
    );
}

/// A later invalid action rolls the entire batch back.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn bulk_action_is_atomic_and_rolls_back_earlier_writes(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-bulk@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Original").await;
    let p1 = create_pending_proposal(app.clone(), &cookie, &chunk_id, "Approved via bulk").await;

    // When
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
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Neither the chunk nor p1 changed despite the first action succeeding
    // before the invalid second action was encountered.
    let chunk = get_chunk(app.clone(), &cookie, &chunk_id).await;
    assert_eq!(chunk["title"], "Original");
    let proposal = json_body(get_proposal(app, &cookie, &p1).await).await;
    assert_eq!(proposal["status"], "pending");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn bulk_action_approves_and_rejects_in_one_call(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-bulk2@b.test", "Alice").await;
    let chunk_id = create_chunk(app.clone(), &cookie, "Chunk").await;
    let p1 = create_pending_proposal(app.clone(), &cookie, &chunk_id, "Approve me").await;
    let p2 = create_pending_proposal(app.clone(), &cookie, &chunk_id, "Reject me").await;

    // When
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
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body[0]["status"], "approved");
    assert_eq!(body[1]["status"], "rejected");
    assert_eq!(body[1]["reviewNote"], "not needed");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn not_found_message_is_titlecase(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-404@b.test", "Alice").await;

    // When
    let res = get_proposal(app.clone(), &cookie, "no-such-id").await;
    // Then
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
    // Given
    let app = fubbik_api::router(state(pool));

    // When
    let res = app
        .clone()
        .oneshot(Request::get("/api/proposals").body(Body::empty()).unwrap())
        .await
        .unwrap();
    // Then
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
