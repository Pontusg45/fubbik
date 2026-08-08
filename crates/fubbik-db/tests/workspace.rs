//! `workspace_space` is the fourth user-scoped many-to-many join in this
//! project (after `chunk_tag`, `chunk_space`, and — trivially, via a
//! single-owner FK — `user_favorite`). Same shape, same two independent
//! attach-side holes as `chunk_tag`/`chunk_space` (`tests/tag.rs`,
//! `tests/space.rs`): a fix for one direction does not fix the other, so
//! each gets its own test. This join additionally exercises a
//! single-row-remove guard (not a replace-set) that needs the identical
//! "prove it in SQL, not just at the service layer" treatment — see
//! `workspace::remove_space`'s doc comment.

use fubbik_db::repo::space::{self, NewSpace};
use fubbik_db::repo::user;
use fubbik_db::repo::workspace::{self, NewWorkspace, WorkspacePatch};

async fn seed(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

fn new_wiki_space(name: &str) -> NewSpace {
    NewSpace {
        name: name.into(),
        kind: "wiki".into(),
        description: None,
    }
}

async fn a_space(pool: &sqlx::PgPool, uid: &str, name: &str) -> String {
    space::create(pool, uid, new_wiki_space(name), None)
        .await
        .unwrap()
        .id
}

fn new_workspace(name: &str) -> NewWorkspace {
    NewWorkspace {
        name: name.into(),
        description: None,
    }
}

async fn a_workspace(pool: &sqlx::PgPool, uid: &str, name: &str) -> String {
    workspace::create(pool, uid, new_workspace(name))
        .await
        .unwrap()
        .id
}

// ── Basic CRUD ──────────────────────────────────────────────────────────

#[sqlx::test]
async fn create_and_find_by_id_round_trip(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let created = workspace::create(
        &pool,
        &alice,
        NewWorkspace {
            name: "platform".into(),
            description: Some("everything".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(created.name, "platform");
    assert_eq!(created.description.as_deref(), Some("everything"));
    assert_eq!(created.user_id, alice);

    let found = workspace::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .expect("must find own workspace");
    assert_eq!(found.id, created.id);
}

#[sqlx::test]
async fn find_by_id_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_ws = a_workspace(&pool, &alice, "alices").await;

    assert!(
        workspace::find_by_id(&pool, &bob, &alices_ws)
            .await
            .unwrap()
            .is_none(),
        "Bob must not be able to look up Alice's workspace by id"
    );
}

#[sqlx::test]
async fn list_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    a_workspace(&pool, &alice, "alices").await;
    a_workspace(&pool, &bob, "bobs").await;

    let alice_list = workspace::list(&pool, &alice).await.unwrap();
    assert_eq!(alice_list.len(), 1);
    assert_eq!(alice_list[0].name, "alices");
}

#[sqlx::test]
async fn update_with_no_fields_is_a_reselect_and_does_not_bump_updated_at(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let created = a_workspace(&pool, &alice, "platform").await;
    let before = workspace::find_by_id(&pool, &alice, &created)
        .await
        .unwrap()
        .unwrap();

    let updated = workspace::update(&pool, &alice, &created, WorkspacePatch::default())
        .await
        .unwrap()
        .expect("no-op patch must still find the row");
    assert_eq!(updated.updated_at, before.updated_at);
}

#[sqlx::test]
async fn update_description_null_clears_it(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let created = workspace::create(
        &pool,
        &alice,
        NewWorkspace {
            name: "platform".into(),
            description: Some("has one".into()),
        },
    )
    .await
    .unwrap();

    let updated = workspace::update(
        &pool,
        &alice,
        &created.id,
        WorkspacePatch {
            name: None,
            description: Some(None),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated.description, None);
}

#[sqlx::test]
async fn update_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_ws = a_workspace(&pool, &alice, "alices").await;

    let result = workspace::update(
        &pool,
        &bob,
        &alices_ws,
        WorkspacePatch {
            name: Some("hijacked".into()),
            description: None,
        },
    )
    .await
    .unwrap();
    assert!(
        result.is_none(),
        "Bob must not be able to update Alice's workspace"
    );

    let still = workspace::find_by_id(&pool, &alice, &alices_ws)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still.name, "alices", "Alice's workspace must be untouched");
}

#[sqlx::test]
async fn delete_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_ws = a_workspace(&pool, &alice, "alices").await;

    let deleted = workspace::delete(&pool, &bob, &alices_ws).await.unwrap();
    assert!(!deleted, "Bob must not be able to delete Alice's workspace");
    assert!(
        workspace::find_by_id(&pool, &alice, &alices_ws)
            .await
            .unwrap()
            .is_some(),
        "Alice's workspace must survive Bob's rejected delete"
    );
}

#[sqlx::test]
async fn delete_cascades_workspace_space_rows(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let ws = a_workspace(&pool, &alice, "platform").await;
    let sp = a_space(&pool, &alice, "code").await;
    workspace::add_space(&pool, &alice, &ws, &sp).await.unwrap();

    workspace::delete(&pool, &alice, &ws).await.unwrap();

    let remaining: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM workspace_space WHERE workspace_id = $1"#,
        ws
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        remaining, 0,
        "workspace_space rows must be cascade-deleted with the workspace"
    );
}

// ── The join: three guards ────────────────────────────────────────────

#[sqlx::test]
async fn cannot_add_another_users_space_to_my_workspace(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_ws = a_workspace(&pool, &alice, "alices-ws").await;
    let bobs_space = a_space(&pool, &bob, "bobs-space").await;

    // Alice tries to attach Bob's space to her own workspace — must be
    // rejected.
    let result = workspace::add_space(&pool, &alice, &alices_ws, &bobs_space)
        .await
        .unwrap();
    assert!(result.is_none(), "must not attach another user's space");
    assert!(
        workspace::spaces_for_workspace(&pool, &alice, &alices_ws)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn cannot_add_my_space_to_another_users_workspace(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let bobs_ws = a_workspace(&pool, &bob, "bobs-ws").await;
    let alices_space = a_space(&pool, &alice, "alices-space").await;

    // Alice tries to put her own space into Bob's workspace — must be
    // rejected.
    let result = workspace::add_space(&pool, &alice, &bobs_ws, &alices_space)
        .await
        .unwrap();
    assert!(
        result.is_none(),
        "must not attach a space to another user's workspace"
    );
    assert!(
        workspace::spaces_for_workspace(&pool, &bob, &bobs_ws)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn own_workspace_and_own_space_succeeds(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let ws = a_workspace(&pool, &alice, "platform").await;
    let sp = a_space(&pool, &alice, "code").await;

    let link = workspace::add_space(&pool, &alice, &ws, &sp)
        .await
        .unwrap()
        .expect("own workspace + own space must succeed");
    assert_eq!(link.workspace_id, ws);
    assert_eq!(link.space_id, sp);

    let spaces = workspace::spaces_for_workspace(&pool, &alice, &ws)
        .await
        .unwrap();
    assert_eq!(spaces.len(), 1);
    assert_eq!(spaces[0].id, sp);
}

#[sqlx::test]
async fn add_space_is_a_silent_no_op_on_duplicate(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let ws = a_workspace(&pool, &alice, "platform").await;
    let sp = a_space(&pool, &alice, "code").await;

    workspace::add_space(&pool, &alice, &ws, &sp)
        .await
        .unwrap()
        .expect("first attach must succeed");

    let second = workspace::add_space(&pool, &alice, &ws, &sp).await.unwrap();
    assert!(
        second.is_none(),
        "duplicate attach must be a silent ON CONFLICT no-op"
    );

    let spaces = workspace::spaces_for_workspace(&pool, &alice, &ws)
        .await
        .unwrap();
    assert_eq!(spaces.len(), 1, "only one workspace_space row may exist");
}

/// The guard that matters most and is easiest to omit: an attacker's
/// *rejected* `remove_space` call must not wipe the victim's pre-existing
/// association. This is a single-row DELETE, not a replace-set, but the
/// risk shape is identical to `chunk_tag`/`chunk_space`'s delete-half bug —
/// see `tests/space.rs::rejected_attach_does_not_wipe_the_victim_chunks_existing_spaces`
/// for the analogous test on that join, and `workspace::remove_space`'s doc
/// comment for why the DELETE itself must carry the ownership check, not
/// just the service layer above it.
#[sqlx::test]
async fn rejected_remove_does_not_wipe_the_victims_existing_association(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_ws = a_workspace(&pool, &alice, "alices-ws").await;
    let alices_space = a_space(&pool, &alice, "alices-space").await;

    // Alice legitimately links her own space to her own workspace.
    workspace::add_space(&pool, &alice, &alices_ws, &alices_space)
        .await
        .unwrap()
        .expect("Alice's own attach must succeed");

    // Bob then tries (and must fail) to remove that association,
    // authenticating as himself but naming Alice's workspace/space ids.
    let removed = workspace::remove_space(&pool, &bob, &alices_ws, &alices_space)
        .await
        .unwrap();
    assert!(!removed, "Bob's remove must be rejected");

    let spaces = workspace::spaces_for_workspace(&pool, &alice, &alices_ws)
        .await
        .unwrap();
    assert_eq!(
        spaces.len(),
        1,
        "Alice's existing workspace/space association must survive Bob's rejected call"
    );
    assert_eq!(spaces[0].id, alices_space);
}

#[sqlx::test]
async fn remove_space_removes_the_callers_own_association(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let ws = a_workspace(&pool, &alice, "platform").await;
    let sp = a_space(&pool, &alice, "code").await;
    workspace::add_space(&pool, &alice, &ws, &sp).await.unwrap();

    let removed = workspace::remove_space(&pool, &alice, &ws, &sp)
        .await
        .unwrap();
    assert!(removed);
    assert!(
        workspace::spaces_for_workspace(&pool, &alice, &ws)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn remove_space_on_nonexistent_link_returns_false(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let ws = a_workspace(&pool, &alice, "platform").await;
    let sp = a_space(&pool, &alice, "code").await;

    let removed = workspace::remove_space(&pool, &alice, &ws, &sp)
        .await
        .unwrap();
    assert!(!removed);
}

// ── Ordering ─────────────────────────────────────────────────────────────

/// Same bug class as `chunk::list`/`tag::list`/`space::list`: `ORDER BY
/// created_at ASC` alone over tied rows is a query-plan artifact. Every
/// workspace here shares the exact same `created_at`, so only the `id ASC`
/// tiebreaker can determine order.
#[sqlx::test]
async fn list_breaks_created_at_ties_by_id(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;

    for name in [
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ] {
        a_workspace(&pool, &alice, name).await;
    }

    sqlx::query!(
        "UPDATE workspace SET created_at = now() WHERE user_id = $1",
        alice
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!("ANALYZE workspace")
        .execute(&pool)
        .await
        .unwrap();

    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM workspace WHERE user_id = $1 ORDER BY id ASC",
        alice
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 20);

    let first = workspace::list(&pool, &alice).await.unwrap();
    let second = workspace::list(&pool, &alice).await.unwrap();

    let first_ids: Vec<String> = first.iter().map(|w| w.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|w| w.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}

/// Same bug class as above, for `spaces_for_workspace`'s `ORDER BY s.name,
/// s.id ASC`. `space.name` is unique per user (`space_user_name_idx`), so a
/// name tie can only be constructed across several distinct users' spaces
/// that happen to share text — this deliberately bypasses `add_space`'s
/// ownership guard with a direct `INSERT INTO workspace_space` purely to
/// construct a genuine tie for the ordering guarantee under test, the same
/// technique `space.rs`'s `spaces_for_chunk_breaks_name_ties_by_id` uses.
#[sqlx::test]
async fn spaces_for_workspace_breaks_name_ties_by_id(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let ws = a_workspace(&pool, &alice, "platform").await;

    for i in 0..5 {
        let owner = seed(&pool, &format!("owner{i}@b.test")).await;
        let sp = a_space(&pool, &owner, "shared-name").await;
        sqlx::query!(
            "INSERT INTO workspace_space (workspace_id, space_id) VALUES ($1, $2)",
            ws,
            sp
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT space_id FROM workspace_space WHERE workspace_id = $1 ORDER BY space_id ASC",
        ws
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 5);

    let first = workspace::spaces_for_workspace(&pool, &alice, &ws)
        .await
        .unwrap();
    let second = workspace::spaces_for_workspace(&pool, &alice, &ws)
        .await
        .unwrap();
    assert!(
        first.iter().all(|s| s.name == "shared-name"),
        "sanity check: the tie must be genuine, not five distinct names"
    );

    let first_ids: Vec<String> = first.iter().map(|s| s.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|s| s.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}
