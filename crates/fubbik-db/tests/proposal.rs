//! `proposal::create` has no `POST /proposals` HTTP route of its own to seed
//! chunks with, so every test here first creates a chunk with `chunk::create`
//! (owned by whichever user the test needs), then a proposal against it with
//! `proposal::create`.
//!
//! Ownership scoping for `approve` lives in `fubbik-api`
//! (`chunks::service::update`, composed with `proposal::update_status` —
//! see `fubbik_db::repo::proposal`'s module doc comment), not here: this
//! repository layer has no `user_id`-scoped proposal query at all except by
//! way of a plain `chunk_id`/`status` filter. The cross-user "approve as
//! non-owner" guard-removal proof lives in
//! `crates/fubbik-api/tests/proposals.rs`.

use fubbik_db::repo::chunk::NewChunk;
use fubbik_db::repo::proposal::{ListProposalsFilter, NewProposal, ProposedChanges};
use fubbik_db::repo::{chunk, proposal, user};

async fn seed_chunk(pool: &sqlx::PgPool, user_id: &str, title: &str) -> String {
    chunk::create(
        pool,
        user_id,
        NewChunk {
            title: title.to_string(),
            content: "original content".to_string(),
            chunk_type: "note".to_string(),
            rationale: None,
        },
    )
    .await
    .unwrap()
    .id
}

fn title_change(title: &str) -> ProposedChanges {
    ProposedChanges {
        title: Some(title.to_string()),
        ..Default::default()
    }
}

#[sqlx::test]
async fn create_then_find_by_id_round_trips_changes_and_reason(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let chunk_id = seed_chunk(&pool, &alice, "Original").await;

    let changes = ProposedChanges {
        title: Some("Renamed".into()),
        rationale: Some("clarity".into()),
        ..Default::default()
    };
    let created = proposal::create(
        &pool,
        NewProposal {
            chunk_id: &chunk_id,
            proposed_by: &alice,
            changes: &changes,
            reason: Some("typo fix"),
        },
    )
    .await
    .unwrap();

    assert_eq!(created.chunk_id, chunk_id);
    assert_eq!(created.status, "pending");
    assert_eq!(created.proposed_by, alice);
    assert_eq!(created.reason.as_deref(), Some("typo fix"));
    assert_eq!(created.changes.0.title.as_deref(), Some("Renamed"));
    assert!(created.reviewed_by.is_none());
    assert!(created.reviewed_at.is_none());

    let found = proposal::find_by_id(&pool, &created.id)
        .await
        .unwrap()
        .expect("just-created proposal must be found by id");
    assert_eq!(found.id, created.id);
    assert_eq!(found.changes.0.title.as_deref(), Some("Renamed"));
}

/// `create` never checks that `chunk_id` exists — see
/// `fubbik_db::repo::proposal::create`'s doc comment. An unknown chunk id
/// fails the foreign key, surfacing as `AppError::Database`, not
/// `AppError::NotFound`.
#[sqlx::test]
async fn create_against_unknown_chunk_fails_the_foreign_key(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let changes = title_change("Renamed");

    let err = proposal::create(
        &pool,
        NewProposal {
            chunk_id: "no-such-chunk",
            proposed_by: &alice,
            changes: &changes,
            reason: None,
        },
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, fubbik_core::error::AppError::Database(_)),
        "an unknown chunk_id must fail the FK, not be pre-checked away as NotFound: {err:?}"
    );
}

/// Global queue: no `user_id` filter, and it is scoped by `status`
/// (defaulted at the service layer, but the repository itself takes a
/// required `status`) plus an optional `chunk_id`. This proves both filters
/// AND the `chunk c INNER JOIN` fields.
#[sqlx::test]
async fn list_filters_by_status_and_chunk_id_and_joins_chunk_fields(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let chunk_a = seed_chunk(&pool, &alice, "Chunk A").await;
    let chunk_b = seed_chunk(&pool, &alice, "Chunk B").await;

    let p1 = proposal::create(
        &pool,
        NewProposal {
            chunk_id: &chunk_a,
            proposed_by: &alice,
            changes: &title_change("A v2"),
            reason: None,
        },
    )
    .await
    .unwrap();
    let p2 = proposal::create(
        &pool,
        NewProposal {
            chunk_id: &chunk_b,
            proposed_by: &alice,
            changes: &title_change("B v2"),
            reason: None,
        },
    )
    .await
    .unwrap();
    proposal::update_status(&pool, &p2.id, "approved", &alice, None)
        .await
        .unwrap();

    // Filtered by status=pending: only p1.
    let pending = proposal::list(
        &pool,
        ListProposalsFilter {
            chunk_id: None,
            status: "pending",
            limit: 50,
            offset: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, p1.id);
    assert_eq!(pending[0].chunk_title, "Chunk A");
    assert_eq!(pending[0].chunk_type, "note");

    // Filtered by status=approved: only p2.
    let approved = proposal::list(
        &pool,
        ListProposalsFilter {
            chunk_id: None,
            status: "approved",
            limit: 50,
            offset: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(approved.len(), 1);
    assert_eq!(approved[0].id, p2.id);

    // chunk_id narrows further.
    let by_chunk = proposal::list(
        &pool,
        ListProposalsFilter {
            chunk_id: Some(chunk_b.as_str()),
            status: "approved",
            limit: 50,
            offset: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(by_chunk.len(), 1);
    assert_eq!(by_chunk[0].id, p2.id);
}

/// `list_for_chunk`'s `status` is unvalidated free text — a nonsense value
/// simply matches zero rows rather than erroring. See
/// `fubbik_db::repo::proposal::list_for_chunk`'s doc comment.
#[sqlx::test]
async fn list_for_chunk_status_is_unvalidated_free_text(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let chunk_id = seed_chunk(&pool, &alice, "Chunk").await;
    proposal::create(
        &pool,
        NewProposal {
            chunk_id: &chunk_id,
            proposed_by: &alice,
            changes: &title_change("v2"),
            reason: None,
        },
    )
    .await
    .unwrap();

    let all = proposal::list_for_chunk(&pool, &chunk_id, None)
        .await
        .unwrap();
    assert_eq!(all.len(), 1);

    let nonsense = proposal::list_for_chunk(&pool, &chunk_id, Some("totally-bogus"))
        .await
        .unwrap();
    assert!(
        nonsense.is_empty(),
        "an unrecognised status must match zero rows, not error"
    );
}

/// `update_status` sets `reviewed_by`/`reviewed_at`/`review_note` and
/// flips `status`, matching Node's `updateProposalStatus`
/// (`packages/db/src/repository/chunk-proposal.ts:80-100`).
#[sqlx::test]
async fn update_status_sets_review_fields(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "c@d.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let chunk_id = seed_chunk(&pool, &alice, "Chunk").await;
    let created = proposal::create(
        &pool,
        NewProposal {
            chunk_id: &chunk_id,
            proposed_by: &alice,
            changes: &title_change("v2"),
            reason: None,
        },
    )
    .await
    .unwrap();

    let updated = proposal::update_status(&pool, &created.id, "rejected", &bob, Some("no thanks"))
        .await
        .unwrap()
        .expect("existing proposal must be found and updated");

    assert_eq!(updated.status, "rejected");
    assert_eq!(updated.reviewed_by.as_deref(), Some(bob.as_str()));
    assert_eq!(updated.review_note.as_deref(), Some("no thanks"));
    assert!(updated.reviewed_at.is_some());
}

#[sqlx::test]
async fn update_status_on_unknown_id_returns_none(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let result = proposal::update_status(&pool, "no-such-proposal", "approved", &alice, None)
        .await
        .unwrap();
    assert!(result.is_none());
}

/// Global count, not scoped to a single user — matches Node's
/// `getPendingCount`. This proves it counts pending proposals across
/// multiple users' chunks and ignores non-pending ones.
#[sqlx::test]
async fn count_pending_counts_across_users_and_ignores_other_statuses(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "c@d.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let alice_chunk = seed_chunk(&pool, &alice, "Alice's chunk").await;
    let bob_chunk = seed_chunk(&pool, &bob, "Bob's chunk").await;

    assert_eq!(proposal::count_pending(&pool).await.unwrap(), 0);

    proposal::create(
        &pool,
        NewProposal {
            chunk_id: &alice_chunk,
            proposed_by: &bob,
            changes: &title_change("v2"),
            reason: None,
        },
    )
    .await
    .unwrap();
    let p2 = proposal::create(
        &pool,
        NewProposal {
            chunk_id: &bob_chunk,
            proposed_by: &alice,
            changes: &title_change("v2"),
            reason: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(proposal::count_pending(&pool).await.unwrap(), 2);

    proposal::update_status(&pool, &p2.id, "approved", &bob, None)
        .await
        .unwrap();
    assert_eq!(proposal::count_pending(&pool).await.unwrap(), 1);
}

/// Same bug class as `notification::list`, `chunk::list`, `tag::list`: an
/// `ORDER BY` over tied `created_at` values with no deterministic
/// tiebreaker is a query-plan artifact. Every proposal here shares the
/// exact same `created_at`, so only the added `id ASC` tiebreaker can
/// determine the global queue's order.
#[sqlx::test]
async fn list_breaks_created_at_ties_by_id(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let chunk_id = seed_chunk(&pool, &alice, "Chunk").await;

    for i in 0..20 {
        proposal::create(
            &pool,
            NewProposal {
                chunk_id: &chunk_id,
                proposed_by: &alice,
                changes: &title_change(&format!("v{i}")),
                reason: None,
            },
        )
        .await
        .unwrap();
    }

    sqlx::query!("UPDATE chunk_proposal SET created_at = now()")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query!("ANALYZE chunk_proposal")
        .execute(&pool)
        .await
        .unwrap();

    let expected_id_order: Vec<String> =
        sqlx::query_scalar!("SELECT id FROM chunk_proposal ORDER BY id ASC")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(expected_id_order.len(), 20);

    let first = proposal::list(
        &pool,
        ListProposalsFilter {
            chunk_id: None,
            status: "pending",
            limit: 50,
            offset: 0,
        },
    )
    .await
    .unwrap();
    let second = proposal::list(
        &pool,
        ListProposalsFilter {
            chunk_id: None,
            status: "pending",
            limit: 50,
            offset: 0,
        },
    )
    .await
    .unwrap();

    let first_ids: Vec<String> = first.iter().map(|p| p.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|p| p.id.clone()).collect();
    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );

    // `list_for_chunk` orders ascending (the opposite direction from the
    // global queue) but must be equally stable under the same tie.
    let for_chunk_first = proposal::list_for_chunk(&pool, &chunk_id, None)
        .await
        .unwrap();
    let for_chunk_second = proposal::list_for_chunk(&pool, &chunk_id, None)
        .await
        .unwrap();
    let for_chunk_first_ids: Vec<String> = for_chunk_first.iter().map(|p| p.id.clone()).collect();
    let for_chunk_second_ids: Vec<String> = for_chunk_second.iter().map(|p| p.id.clone()).collect();
    assert_eq!(for_chunk_first_ids, for_chunk_second_ids);
    let mut expected_asc = expected_id_order.clone();
    expected_asc.sort();
    assert_eq!(for_chunk_first_ids, expected_asc);
}
