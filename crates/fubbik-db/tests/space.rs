//! `chunk_space` is a composite-key join with no `user_id` of its own —
//! ownership derives entirely from the two parent rows (`chunk`, `space`).
//! Same shape, same two independent holes, as `chunk_tag` (`tests/tag.rs`):
//! a fix for one direction does not fix the other, so each gets its own
//! test, plus a delete-half test that `tests/tag.rs` did not have (see the
//! doc comment on `space::set_chunk_spaces` for why that half matters).

use fubbik_db::repo::space::{self, CodeInput, CodeUpdate, NewSpace, SpacePatch};
use fubbik_db::repo::{chunk, user};

async fn seed(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn a_chunk(pool: &sqlx::PgPool, uid: &str, title: &str) -> String {
    chunk::create(
        pool,
        uid,
        chunk::NewChunk {
            title: title.into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap()
    .id
}

fn new_wiki_space(name: &str) -> NewSpace {
    NewSpace {
        name: name.into(),
        kind: "wiki".into(),
        description: None,
    }
}

fn new_code_space(name: &str) -> NewSpace {
    NewSpace {
        name: name.into(),
        kind: "code".into(),
        description: None,
    }
}

#[sqlx::test]
async fn create_then_list_then_detail_round_trip(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;

    let created = space::create(&pool, &alice, new_wiki_space("notes"), None)
        .await
        .unwrap();
    assert_eq!(created.name, "notes");
    assert_eq!(created.kind, "wiki");
    assert_eq!(created.description, None);

    let listed = space::list(&pool, &alice).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let detail = space::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .expect("must find own space");
    assert_eq!(detail.space.id, created.id);
    assert!(
        detail.code.is_none(),
        "a non-code-kind space must have no space_code_metadata row"
    );
}

#[sqlx::test]
async fn code_kind_space_round_trips_remote_url_and_local_paths(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;

    let created = space::create(
        &pool,
        &alice,
        new_code_space("fubbik"),
        Some(CodeInput {
            remote_url: Some("github.com/acme/fubbik".into()),
            local_paths: vec!["/Users/alice/fubbik".into()],
        }),
    )
    .await
    .unwrap();

    let detail = space::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .expect("must find own space");
    let code = detail.code.expect("code-kind space must have metadata");
    assert_eq!(code.remote_url.as_deref(), Some("github.com/acme/fubbik"));
    assert_eq!(code.local_paths.0, vec!["/Users/alice/fubbik".to_string()]);

    // List never joins space_code_metadata — bare rows only.
    let listed = space::list(&pool, &alice).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
}

#[sqlx::test]
async fn code_kind_space_with_no_remote_url_or_local_paths_still_gets_a_metadata_row(
    pool: sqlx::PgPool,
) {
    let alice = seed(&pool, "a@b.test").await;

    // Matches Node: `code: kind === "code" ? {...} : undefined` keys off
    // `kind` alone, so a code-kind space always gets a side-table row, even
    // with no remoteUrl/localPaths given.
    let created = space::create(
        &pool,
        &alice,
        new_code_space("empty-code"),
        Some(CodeInput {
            remote_url: None,
            local_paths: vec![],
        }),
    )
    .await
    .unwrap();

    let detail = space::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .unwrap();
    let code = detail.code.expect("must still get a metadata row");
    assert_eq!(code.remote_url, None);
    assert_eq!(code.local_paths.0, Vec::<String>::new());
}

#[sqlx::test]
async fn detect_finds_code_space_by_remote_url_and_local_path(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;

    let created = space::create(
        &pool,
        &alice,
        new_code_space("fubbik"),
        Some(CodeInput {
            remote_url: Some("github.com/acme/fubbik".into()),
            local_paths: vec!["/Users/alice/fubbik".into()],
        }),
    )
    .await
    .unwrap();

    let by_url = space::find_by_remote_url(&pool, &alice, "github.com/acme/fubbik")
        .await
        .unwrap()
        .expect("must find by exact normalized remote url");
    assert_eq!(by_url.id, created.id);

    let by_path = space::find_by_local_path(&pool, &alice, "/Users/alice/fubbik")
        .await
        .unwrap()
        .expect("must find by contained local path");
    assert_eq!(by_path.id, created.id);

    assert!(
        space::find_by_remote_url(&pool, &alice, "github.com/acme/other")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        space::find_by_local_path(&pool, &alice, "/Users/alice/other")
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test]
async fn detect_is_scoped_to_the_caller(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;

    space::create(
        &pool,
        &alice,
        new_code_space("fubbik"),
        Some(CodeInput {
            remote_url: Some("github.com/acme/fubbik".into()),
            local_paths: vec!["/Users/alice/fubbik".into()],
        }),
    )
    .await
    .unwrap();

    assert!(
        space::find_by_remote_url(&pool, &bob, "github.com/acme/fubbik")
            .await
            .unwrap()
            .is_none(),
        "Bob must not detect Alice's space by her remote url"
    );
    assert!(
        space::find_by_local_path(&pool, &bob, "/Users/alice/fubbik")
            .await
            .unwrap()
            .is_none(),
        "Bob must not detect Alice's space by her local path"
    );
}

#[sqlx::test]
async fn update_name_and_description_round_trips(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let created = space::create(&pool, &alice, new_wiki_space("notes"), None)
        .await
        .unwrap();

    let updated = space::update(
        &pool,
        &alice,
        &created.id,
        SpacePatch {
            name: Some("renamed".into()),
            description: Some(Some("a description".into())),
        },
        None,
    )
    .await
    .unwrap()
    .expect("must find and update own space");
    assert_eq!(updated.name, "renamed");
    assert_eq!(updated.description.as_deref(), Some("a description"));
    assert!(
        updated.updated_at.0 >= created.updated_at.0,
        "updated_at must not go backwards"
    );
}

#[sqlx::test]
async fn update_with_no_fields_present_is_a_plain_select_not_a_no_op_update(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let created = space::create(&pool, &alice, new_wiki_space("notes"), None)
        .await
        .unwrap();

    // Neither name nor description present in the patch: Node falls back to
    // a plain SELECT and does NOT bump updated_at.
    let untouched = space::update(&pool, &alice, &created.id, SpacePatch::default(), None)
        .await
        .unwrap()
        .expect("must still return the row");
    assert_eq!(untouched.name, created.name);
    assert_eq!(untouched.updated_at.0, created.updated_at.0);
}

#[sqlx::test]
async fn update_description_explicit_null_clears_it(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let created = space::create(
        &pool,
        &alice,
        NewSpace {
            name: "notes".into(),
            kind: "wiki".into(),
            description: Some("original".into()),
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(created.description.as_deref(), Some("original"));

    // Omitted description must leave it untouched.
    let untouched = space::update(
        &pool,
        &alice,
        &created.id,
        SpacePatch {
            name: Some("renamed".into()),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(untouched.description.as_deref(), Some("original"));

    // Explicit null must clear it.
    let cleared = space::update(
        &pool,
        &alice,
        &created.id,
        SpacePatch {
            name: None,
            description: Some(None),
        },
        None,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(cleared.description, None);
    assert_eq!(
        cleared.name, "renamed",
        "clearing description must not touch the name set by the previous update"
    );
}

#[sqlx::test]
async fn update_upserts_code_metadata_for_code_kind_space(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let created = space::create(
        &pool,
        &alice,
        new_code_space("fubbik"),
        Some(CodeInput {
            remote_url: Some("github.com/acme/fubbik".into()),
            local_paths: vec!["/Users/alice/fubbik".into()],
        }),
    )
    .await
    .unwrap();

    space::update(
        &pool,
        &alice,
        &created.id,
        SpacePatch::default(),
        Some(CodeUpdate {
            remote_url: Some(Some("github.com/acme/renamed".into())),
            local_paths: Some(vec!["/Users/alice/renamed".into()]),
        }),
    )
    .await
    .unwrap();

    let detail = space::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .unwrap();
    let code = detail.code.unwrap();
    assert_eq!(code.remote_url.as_deref(), Some("github.com/acme/renamed"));
    assert_eq!(code.local_paths.0, vec!["/Users/alice/renamed".to_string()]);
}

/// DELIBERATE DIVERGENCE FROM NODE (#3 in this slice): `code: None` at the
/// repo layer must leave `space_code_metadata` completely untouched — no
/// upsert at all — not clear it to `null`/`[]`. This is the repo-level half
/// of the fix; `spaces::service::update` is the other half, deciding when
/// to pass `None` vs. `Some(CodeUpdate{..})`.
#[sqlx::test]
async fn update_with_no_code_param_leaves_code_metadata_untouched(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let created = space::create(
        &pool,
        &alice,
        new_code_space("fubbik"),
        Some(CodeInput {
            remote_url: Some("github.com/acme/fubbik".into()),
            local_paths: vec!["/Users/alice/fubbik".into()],
        }),
    )
    .await
    .unwrap();

    space::update(
        &pool,
        &alice,
        &created.id,
        SpacePatch {
            name: Some("renamed".into()),
            description: None,
        },
        None,
    )
    .await
    .unwrap();

    let detail = space::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(detail.space.name, "renamed");
    let code = detail.code.unwrap();
    assert_eq!(code.remote_url.as_deref(), Some("github.com/acme/fubbik"));
    assert_eq!(code.local_paths.0, vec!["/Users/alice/fubbik".to_string()]);
}

/// Each of `remote_url`/`local_paths` is independently settable — providing
/// one must not clobber the other back to `null`/`[]`.
#[sqlx::test]
async fn update_code_metadata_fields_are_independently_settable(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let created = space::create(
        &pool,
        &alice,
        new_code_space("fubbik"),
        Some(CodeInput {
            remote_url: Some("github.com/acme/fubbik".into()),
            local_paths: vec!["/Users/alice/fubbik".into()],
        }),
    )
    .await
    .unwrap();

    // Only remote_url given: local_paths must survive untouched.
    space::update(
        &pool,
        &alice,
        &created.id,
        SpacePatch::default(),
        Some(CodeUpdate {
            remote_url: Some(Some("github.com/acme/renamed".into())),
            local_paths: None,
        }),
    )
    .await
    .unwrap();

    let detail = space::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .unwrap();
    let code = detail.code.unwrap();
    assert_eq!(code.remote_url.as_deref(), Some("github.com/acme/renamed"));
    assert_eq!(code.local_paths.0, vec!["/Users/alice/fubbik".to_string()]);

    // Only local_paths given: remote_url must survive untouched.
    space::update(
        &pool,
        &alice,
        &created.id,
        SpacePatch::default(),
        Some(CodeUpdate {
            remote_url: None,
            local_paths: Some(vec!["/Users/alice/renamed".into()]),
        }),
    )
    .await
    .unwrap();

    let detail = space::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .unwrap();
    let code = detail.code.unwrap();
    assert_eq!(
        code.remote_url.as_deref(),
        Some("github.com/acme/renamed"),
        "remote_url must survive an update that only touches local_paths"
    );
    assert_eq!(code.local_paths.0, vec!["/Users/alice/renamed".to_string()]);
}

/// Explicit `null` still clears `remote_url` — the tri-state's `Some(None)`
/// case. Only *omission* is exempted by the fix; an explicit clear request
/// still works.
#[sqlx::test]
async fn update_code_metadata_remote_url_explicit_none_clears_it(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let created = space::create(
        &pool,
        &alice,
        new_code_space("fubbik"),
        Some(CodeInput {
            remote_url: Some("github.com/acme/fubbik".into()),
            local_paths: vec!["/Users/alice/fubbik".into()],
        }),
    )
    .await
    .unwrap();

    space::update(
        &pool,
        &alice,
        &created.id,
        SpacePatch::default(),
        Some(CodeUpdate {
            remote_url: Some(None),
            local_paths: None,
        }),
    )
    .await
    .unwrap();

    let detail = space::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .unwrap();
    let code = detail.code.unwrap();
    assert_eq!(code.remote_url, None);
    assert_eq!(
        code.local_paths.0,
        vec!["/Users/alice/fubbik".to_string()],
        "local_paths must survive since it was not part of this update"
    );
}

#[sqlx::test]
async fn cross_user_cannot_see_update_or_delete_a_space(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let created = space::create(&pool, &alice, new_wiki_space("notes"), None)
        .await
        .unwrap();

    assert!(
        space::find_by_id(&pool, &bob, &created.id)
            .await
            .unwrap()
            .is_none(),
        "Bob must not see Alice's space"
    );
    assert!(space::list(&pool, &bob).await.unwrap().is_empty());

    let update_result = space::update(
        &pool,
        &bob,
        &created.id,
        SpacePatch {
            name: Some("hijacked".into()),
            description: None,
        },
        None,
    )
    .await
    .unwrap();
    assert!(
        update_result.is_none(),
        "Bob must not be able to update Alice's space"
    );

    let deleted = space::delete(&pool, &bob, &created.id).await.unwrap();
    assert!(!deleted, "Bob must not be able to delete Alice's space");

    let still_there = space::find_by_id(&pool, &alice, &created.id).await.unwrap();
    assert!(
        still_there.is_some(),
        "Alice's space must survive Bob's rejected update and delete"
    );
    assert_eq!(still_there.unwrap().space.name, "notes");
}

#[sqlx::test]
async fn delete_removes_the_space(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let created = space::create(&pool, &alice, new_wiki_space("notes"), None)
        .await
        .unwrap();

    let deleted = space::delete(&pool, &alice, &created.id).await.unwrap();
    assert!(deleted);
    assert!(
        space::find_by_id(&pool, &alice, &created.id)
            .await
            .unwrap()
            .is_none()
    );
}

// --- chunk_space join --------------------------------------------------

#[sqlx::test]
async fn cannot_put_another_users_chunk_into_a_space(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let bobs_space = space::create(&pool, &bob, new_wiki_space("bobs"), None)
        .await
        .unwrap();

    // Bob attempts to put Alice's chunk into his own space — must be
    // rejected.
    let n = space::set_chunk_spaces(
        &pool,
        &bob,
        &alices_chunk,
        std::slice::from_ref(&bobs_space.id),
    )
    .await
    .unwrap();
    assert_eq!(n, 0, "must not attach another user's chunk to a space");
    assert!(
        space::spaces_for_chunk(&pool, &alice, &alices_chunk)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn cannot_put_a_chunk_into_another_users_space(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let bobs_space = space::create(&pool, &bob, new_wiki_space("bobs"), None)
        .await
        .unwrap();

    // Alice attempts to put her own chunk into Bob's space — must be
    // rejected.
    let n = space::set_chunk_spaces(
        &pool,
        &alice,
        &alices_chunk,
        std::slice::from_ref(&bobs_space.id),
    )
    .await
    .unwrap();
    assert_eq!(n, 0, "must not attach a chunk to another user's space");
    assert!(
        space::spaces_for_chunk(&pool, &alice, &alices_chunk)
            .await
            .unwrap()
            .is_empty()
    );
}

/// The guard that `tests/tag.rs` did not exercise: an attacker's *rejected*
/// call must not wipe the victim's *pre-existing* `chunk_space` rows via the
/// delete half of the replace-set operation. Both `cannot_put_*` tests above
/// only prove nothing new was inserted; this proves nothing old was removed
/// either — a materially different assertion, since the delete and the
/// insert are guarded independently in the SQL.
#[sqlx::test]
async fn rejected_attach_does_not_wipe_the_victim_chunks_existing_spaces(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let alices_space = space::create(&pool, &alice, new_wiki_space("alices"), None)
        .await
        .unwrap();
    let bobs_space = space::create(&pool, &bob, new_wiki_space("bobs"), None)
        .await
        .unwrap();

    // Alice legitimately puts her chunk in her own space first.
    let n = space::set_chunk_spaces(
        &pool,
        &alice,
        &alices_chunk,
        std::slice::from_ref(&alices_space.id),
    )
    .await
    .unwrap();
    assert_eq!(n, 1);

    // Bob then tries (and must fail) to overwrite Alice's chunk's space set
    // with his own space, impersonating Alice's chunk id but authenticating
    // as himself.
    let n = space::set_chunk_spaces(
        &pool,
        &bob,
        &alices_chunk,
        std::slice::from_ref(&bobs_space.id),
    )
    .await
    .unwrap();
    assert_eq!(n, 0);

    let spaces = space::spaces_for_chunk(&pool, &alice, &alices_chunk)
        .await
        .unwrap();
    assert_eq!(
        spaces.len(),
        1,
        "Alice's existing space association must survive Bob's rejected call"
    );
    assert_eq!(spaces[0].id, alices_space.id);
}

#[sqlx::test]
async fn own_chunk_and_own_space_succeeds_and_set_replaces_the_whole_set(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let space_one = space::create(&pool, &alice, new_wiki_space("one"), None)
        .await
        .unwrap();
    let space_two = space::create(&pool, &alice, new_wiki_space("two"), None)
        .await
        .unwrap();

    space::set_chunk_spaces(
        &pool,
        &alice,
        &alices_chunk,
        std::slice::from_ref(&space_one.id),
    )
    .await
    .unwrap();
    let n = space::set_chunk_spaces(
        &pool,
        &alice,
        &alices_chunk,
        std::slice::from_ref(&space_two.id),
    )
    .await
    .unwrap();
    assert_eq!(n, 1);

    let spaces = space::spaces_for_chunk(&pool, &alice, &alices_chunk)
        .await
        .unwrap();
    assert_eq!(
        spaces.len(),
        1,
        "old space must be replaced, not accumulated"
    );
    assert_eq!(spaces[0].id, space_two.id);
}

// --- reset ---------------------------------------------------------------

#[sqlx::test]
async fn reset_deletes_exclusive_chunks_but_spares_shared_ones(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let space_a = space::create(&pool, &alice, new_wiki_space("a"), None)
        .await
        .unwrap();
    let space_b = space::create(&pool, &alice, new_wiki_space("b"), None)
        .await
        .unwrap();

    let exclusive_chunk = a_chunk(&pool, &alice, "exclusive").await;
    let shared_chunk = a_chunk(&pool, &alice, "shared").await;

    space::set_chunk_spaces(
        &pool,
        &alice,
        &exclusive_chunk,
        std::slice::from_ref(&space_a.id),
    )
    .await
    .unwrap();
    // shared_chunk belongs to BOTH space_a and space_b — set_chunk_spaces
    // replaces the whole set, so both ids must be given in one call.
    space::set_chunk_spaces(
        &pool,
        &alice,
        &shared_chunk,
        &[space_a.id.clone(), space_b.id.clone()],
    )
    .await
    .unwrap();

    let result = space::reset(&pool, &alice, &space_a.id).await.unwrap();
    assert_eq!(result.chunks_deleted, 1, "only the exclusive chunk counts");

    assert!(
        chunk::find_by_id(&pool, &alice, &exclusive_chunk)
            .await
            .unwrap()
            .is_none(),
        "exclusive chunk must be hard-deleted"
    );
    assert!(
        chunk::find_by_id(&pool, &alice, &shared_chunk)
            .await
            .unwrap()
            .is_some(),
        "shared chunk must survive because it still belongs to space_b"
    );

    // shared_chunk loses its association with space_a but keeps space_b.
    let remaining_spaces = space::spaces_for_chunk(&pool, &alice, &shared_chunk)
        .await
        .unwrap();
    assert_eq!(remaining_spaces.len(), 1);
    assert_eq!(remaining_spaces[0].id, space_b.id);

    // The space row and its code metadata (if any) survive a reset.
    assert!(
        space::find_by_id(&pool, &alice, &space_a.id)
            .await
            .unwrap()
            .is_some(),
        "reset must not delete the space row itself"
    );
}

#[sqlx::test]
async fn reset_deletes_space_scoped_documents_plans_and_requirements(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let space = space::create(&pool, &alice, new_wiki_space("a"), None)
        .await
        .unwrap();

    let doc_id = fubbik_db::new_id();
    sqlx::query!(
        "INSERT INTO document (id, title, source_path, content_hash, space_id, user_id) \
           VALUES ($1, 'Doc', '/doc.md', 'hash', $2, $3)",
        doc_id,
        space.id,
        alice
    )
    .execute(&pool)
    .await
    .unwrap();

    let plan_id = fubbik_db::new_id();
    sqlx::query!(
        "INSERT INTO plan (id, title, space_id, user_id) VALUES ($1, 'Plan', $2, $3)",
        plan_id,
        space.id,
        alice
    )
    .execute(&pool)
    .await
    .unwrap();

    let req_id = fubbik_db::new_id();
    sqlx::query!(
        "INSERT INTO requirement (id, title, steps, space_id, user_id) \
           VALUES ($1, 'Req', '[]'::jsonb, $2, $3)",
        req_id,
        space.id,
        alice
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = space::reset(&pool, &alice, &space.id).await.unwrap();
    assert_eq!(result.docs_deleted, 1);
    assert_eq!(result.plans_deleted, 1);
    assert_eq!(result.requirements_deleted, 1);

    let doc_count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM document WHERE id = $1"#,
        doc_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(doc_count, 0);
}

#[sqlx::test]
async fn reset_of_another_users_space_is_a_no_op(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_space = space::create(&pool, &alice, new_wiki_space("a"), None)
        .await
        .unwrap();
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    space::set_chunk_spaces(
        &pool,
        &alice,
        &alices_chunk,
        std::slice::from_ref(&alices_space.id),
    )
    .await
    .unwrap();

    let result = space::reset(&pool, &bob, &alices_space.id).await.unwrap();
    assert_eq!(result.chunks_deleted, 0);
    assert_eq!(result.docs_deleted, 0);
    assert_eq!(result.plans_deleted, 0);
    assert_eq!(result.requirements_deleted, 0);

    assert!(
        chunk::find_by_id(&pool, &alice, &alices_chunk)
            .await
            .unwrap()
            .is_some(),
        "Alice's chunk must survive Bob's reset attempt on her space"
    );
    let spaces = space::spaces_for_chunk(&pool, &alice, &alices_chunk)
        .await
        .unwrap();
    assert_eq!(
        spaces.len(),
        1,
        "chunk_space association must survive Bob's reset attempt"
    );
}

/// Same bug class as `chunk::list` and `tag::list` (see their equivalent
/// tests): `ORDER BY created_at ASC` alone over tied rows is a query-plan
/// artifact. Every space here shares the exact same `created_at`, so only
/// the `id ASC` tiebreaker can determine order.
#[sqlx::test]
async fn list_breaks_created_at_ties_by_id(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;

    for name in ["one", "two", "three", "four", "five"] {
        space::create(&pool, &alice, new_wiki_space(name), None)
            .await
            .unwrap();
    }

    sqlx::query!(
        "UPDATE space SET created_at = now() WHERE user_id = $1",
        alice
    )
    .execute(&pool)
    .await
    .unwrap();

    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM space WHERE user_id = $1 ORDER BY id ASC",
        alice
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 5);

    let first = space::list(&pool, &alice).await.unwrap();
    let second = space::list(&pool, &alice).await.unwrap();

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
