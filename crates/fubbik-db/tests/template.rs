//! Repo-level tests for `chunk_template` — see `fubbik_db::repo::template`'s
//! module doc for the two SQL guards this covers: `update`/`delete`'s
//! `WHERE ... AND user_id = $2` (cross-user protection, which also
//! incidentally protects built-in rows since their `user_id` is always
//! `NULL`) and `delete`'s additional explicit `AND is_built_in = false`.
//!
//! The built-in-rejection *service-layer* check (`ValidationError` /
//! 400 — the check Node's `updateTemplate`/`deleteTemplate` actually make)
//! is tested at the HTTP level in `fubbik-api/tests/templates.rs`, per the
//! rule that an API-level test can't observe a removed SQL guard when a
//! service pre-check would 404 first — these repo tests exist precisely to
//! cover what that API test can't.

use fubbik_db::repo::template::{
    self, ExtractionTarget, FieldMapping, FrontmatterMatchMode, FrontmatterRule, HeadingRule,
    MatchMode, MatchRules, NewTemplate, TemplatePatch,
};
use fubbik_db::repo::user;

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

fn sample_match_rules() -> MatchRules {
    MatchRules {
        min_score: 1.5,
        headings: vec![HeadingRule {
            patterns: vec!["Rationale".into(), "Why".into()],
            match_mode: MatchMode::Prefix,
            level: Some(2),
            required: true,
        }],
        frontmatter: vec![FrontmatterRule {
            key: "status".into(),
            match_mode: FrontmatterMatchMode::OneOf,
            value: None,
            values: Some(vec!["draft".into(), "final".into()]),
        }],
    }
}

fn sample_field_mappings() -> Vec<FieldMapping> {
    vec![FieldMapping {
        headings: vec!["Rationale".into()],
        match_mode: MatchMode::Contains,
        target: ExtractionTarget::Rationale,
    }]
}

async fn seed_builtin(pool: &sqlx::PgPool, name: &str) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO chunk_template (id, name, type, content, is_built_in, user_id)
           VALUES ($1, $2, 'note', '', true, NULL)"#,
        id,
        name
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

#[sqlx::test]
async fn create_and_find_by_id_round_trip(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;

    // When
    let created = template::create(
        &pool,
        &uid,
        NewTemplate {
            name: "ADR".into(),
            description: Some("Architecture decision".into()),
            template_type: "reference".into(),
            content: "## Rationale".into(),
            match_rules: Some(sample_match_rules()),
            field_mappings: Some(sample_field_mappings()),
            priority: Some(5),
            tags: Some(vec!["adr".into(), "decision".into()]),
        },
    )
    .await
    .unwrap();

    // Then
    assert_eq!(created.name, "ADR");
    assert_eq!(
        created.description.as_deref(),
        Some("Architecture decision")
    );
    assert_eq!(created.template_type, "reference");
    assert!(
        !created.is_built_in,
        "create must never produce a built-in row"
    );
    assert_eq!(created.priority, 5);
    assert_eq!(created.tags, Some(vec!["adr".into(), "decision".into()]));
    assert_eq!(created.user_id.as_deref(), Some(uid.as_str()));
    let mr = created.match_rules.as_ref().unwrap();
    assert_eq!(mr.0.min_score, 1.5);
    assert_eq!(mr.0.headings[0].match_mode, MatchMode::Prefix);
    assert_eq!(mr.0.headings[0].level, Some(2));
    let fm = created.field_mappings.as_ref().unwrap();
    assert_eq!(fm.0[0].target, ExtractionTarget::Rationale);

    let found = template::find_by_id(&pool, &created.id)
        .await
        .unwrap()
        .expect("must round-trip");
    assert_eq!(found.id, created.id);
    assert_eq!(found.name, "ADR");
}

/// `matchRules`/`fieldMappings` must serialise with the exact camelCase
/// keys Node's Elysia schema uses (`minScore`, `match`, not `match_mode`,
/// etc.) — a `#[serde(rename = "match")]` slip here would silently break
/// the wire contract without failing compilation.
#[sqlx::test]
async fn match_rules_serialise_with_nodes_exact_json_keys(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let created = template::create(
        &pool,
        &uid,
        NewTemplate {
            name: "ADR".into(),
            description: None,
            template_type: "reference".into(),
            content: "".into(),
            match_rules: Some(sample_match_rules()),
            field_mappings: None,
            priority: None,
            tags: None,
        },
    )
    .await
    .unwrap();

    // When
    let raw: serde_json::Value = sqlx::query_scalar!(
        r#"SELECT match_rules AS "match_rules!: serde_json::Value" FROM chunk_template WHERE id = $1"#,
        created.id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // Then
    assert_eq!(raw["minScore"], serde_json::json!(1.5));
    assert_eq!(raw["headings"][0]["match"], serde_json::json!("prefix"));
    assert_eq!(raw["headings"][0]["required"], serde_json::json!(true));
    assert_eq!(
        raw["frontmatter"][0]["match"],
        serde_json::json!("oneOf"),
        "FrontmatterMatchMode::OneOf must serialise as \"oneOf\", matching Node's t.Literal(\"oneOf\")"
    );
}

/// `create` always defaults `priority` to `0` and `tags` to `NULL` when
/// omitted, matching Node's `params.priority ?? 0` / `params.tags ?? null`
/// (`packages/db/src/repository/template.ts:48-49`).
#[sqlx::test]
async fn create_defaults_priority_and_tags_when_omitted(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    // When
    let created = template::create(
        &pool,
        &uid,
        NewTemplate {
            name: "Bare".into(),
            description: None,
            template_type: "note".into(),
            content: "".into(),
            match_rules: None,
            field_mappings: None,
            priority: None,
            tags: None,
        },
    )
    .await
    .unwrap();
    // Then
    assert_eq!(created.priority, 0);
    assert_eq!(created.tags, None);
    assert_eq!(created.description, None);
    assert_eq!(created.match_rules.map(|j| j.0), None);
}

/// `list` unions built-in templates with the caller's own and excludes
/// every other user's — proves both halves of `WHERE is_built_in = true OR
/// user_id = $1` are load-bearing (dropping either half breaks one of the
/// two assertions below).
#[sqlx::test]
async fn list_includes_builtin_and_own_but_excludes_other_users(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;

    template::create(
        &pool,
        &alice,
        NewTemplate {
            name: "Alice's".into(),
            description: None,
            template_type: "note".into(),
            content: "".into(),
            match_rules: None,
            field_mappings: None,
            priority: None,
            tags: None,
        },
    )
    .await
    .unwrap();
    template::create(
        &pool,
        &bob,
        NewTemplate {
            name: "Bob's".into(),
            description: None,
            template_type: "note".into(),
            content: "".into(),
            match_rules: None,
            field_mappings: None,
            priority: None,
            tags: None,
        },
    )
    .await
    .unwrap();
    seed_builtin(&pool, "Convention").await;

    // When
    let alices_view = template::list(&pool, &alice).await.unwrap();
    let names: Vec<&str> = alices_view.iter().map(|t| t.name.as_str()).collect();
    // Then
    assert!(names.contains(&"Alice's"), "must see own template");
    assert!(names.contains(&"Convention"), "must see built-in template");
    assert!(
        !names.contains(&"Bob's"),
        "must not see another user's template"
    );
}

/// Same "force a genuine tie, then verify Postgres's own ascending-id
/// order" shape as `collection.rs::list_breaks_name_ties_by_id_and_is_stable`.
/// `template_user_name_idx` (`UNIQUE (user_id, name) WHERE user_id IS NOT
/// NULL`) makes a name tie within one user's own templates unreachable
/// through `template::create` itself, so this drops that index for the
/// duration of this test's own isolated database to construct the tie
/// `, id ASC` exists to resolve.
#[sqlx::test]
async fn list_breaks_name_ties_by_id_and_is_stable(pool: sqlx::PgPool) {
    // Given
    sqlx::query!("DROP INDEX template_user_name_idx")
        .execute(&pool)
        .await
        .unwrap();

    let uid = seed_user(&pool, "a@b.test").await;
    for _ in 0..20 {
        template::create(
            &pool,
            &uid,
            NewTemplate {
                name: "Tie".into(),
                description: None,
                template_type: "note".into(),
                content: "".into(),
                match_rules: None,
                field_mappings: None,
                priority: None,
                tags: None,
            },
        )
        .await
        .unwrap();
    }

    let expected_ids: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM chunk_template WHERE user_id = $1 ORDER BY id ASC",
        uid
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let first = template::list(&pool, &uid).await.unwrap();
    // When
    let second = template::list(&pool, &uid).await.unwrap();
    let first_ids: Vec<String> = first.iter().map(|t| t.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|t| t.id.clone()).collect();

    // Then
    assert_eq!(
        first_ids, expected_ids,
        "must match Postgres's own id ASC order"
    );
    assert_eq!(
        first_ids, second_ids,
        "repeated calls must return the same order"
    );
}

/// `update`'s `WHERE user_id = $2` guard, proven with a genuine cross-user
/// attempt: Bob's call must return `None` and Alice's row must be
/// byte-for-byte unchanged (not merely "still exists").
#[sqlx::test]
async fn update_is_scoped_to_owner(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let created = template::create(
        &pool,
        &alice,
        NewTemplate {
            name: "Alice's".into(),
            description: Some("original".into()),
            template_type: "note".into(),
            content: "orig content".into(),
            match_rules: None,
            field_mappings: None,
            priority: None,
            tags: None,
        },
    )
    .await
    .unwrap();

    // When
    let result = template::update(
        &pool,
        &bob,
        &created.id,
        TemplatePatch {
            name: Some("hijacked".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Then
    assert!(
        result.is_none(),
        "Bob must not be able to update Alice's template"
    );

    let still_alices = template::find_by_id(&pool, &created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        still_alices.name, "Alice's",
        "Alice's row must be unaffected"
    );
    assert_eq!(still_alices.description.as_deref(), Some("original"));
}

/// Sanity: the owner's own `update` succeeds and the tri-state
/// `Some(None)` patch fields genuinely clear the column while omitted
/// (`None`) fields are left untouched — matching Node's conditional
/// spread (`packages/db/src/repository/template.ts:74-83`).
#[sqlx::test]
async fn update_owner_succeeds_and_tri_state_clear_vs_untouched(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let created = template::create(
        &pool,
        &uid,
        NewTemplate {
            name: "Original".into(),
            description: Some("desc".into()),
            template_type: "note".into(),
            content: "content".into(),
            match_rules: Some(sample_match_rules()),
            field_mappings: None,
            priority: Some(1),
            tags: None,
        },
    )
    .await
    .unwrap();

    // When
    // Explicit clear of description and match_rules; content untouched
    // (`None`); name set to a new value.
    let updated = template::update(
        &pool,
        &uid,
        &created.id,
        TemplatePatch {
            name: Some("Renamed".into()),
            description: Some(None),
            template_type: None,
            content: None,
            match_rules: Some(None),
            field_mappings: None,
            priority: None,
            tags: None,
        },
    )
    .await
    .unwrap()
    .expect("owner update must succeed");

    // Then
    assert_eq!(updated.name, "Renamed");
    assert_eq!(
        updated.description, None,
        "explicit null must clear description"
    );
    assert_eq!(
        updated.content, "content",
        "omitted content must stay untouched"
    );
    assert_eq!(
        updated.match_rules.map(|j| j.0),
        None,
        "explicit null must clear match_rules"
    );
    assert_eq!(updated.priority, 1, "omitted priority must stay untouched");
}

/// `delete`'s `WHERE user_id = $2` guard, same cross-user shape as
/// `update_is_scoped_to_owner`.
#[sqlx::test]
async fn delete_is_scoped_to_owner(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let created = template::create(
        &pool,
        &alice,
        NewTemplate {
            name: "Alice's".into(),
            description: None,
            template_type: "note".into(),
            content: "".into(),
            match_rules: None,
            field_mappings: None,
            priority: None,
            tags: None,
        },
    )
    .await
    .unwrap();

    // When
    let deleted = template::delete(&pool, &bob, &created.id).await.unwrap();
    // Then
    assert!(!deleted, "Bob must not be able to delete Alice's template");

    let still_there = template::find_by_id(&pool, &created.id).await.unwrap();
    assert!(still_there.is_some(), "Alice's row must still exist");
}

/// Isolates the explicit `AND is_built_in = false` guard in `delete` from
/// the `user_id` guard: this row is a state the app itself never
/// produces (`is_built_in = true` with a real, matching `user_id`) —
/// constructed here purely so a delete call whose `user_id` *does* match
/// can only be blocked by the `is_built_in` check. Removing that clause
/// from `template::delete`'s SQL turns this `assert!(!deleted)` into a
/// failure.
#[sqlx::test]
async fn delete_excludes_built_in_even_when_user_id_matches(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO chunk_template (id, name, type, content, is_built_in, user_id)
           VALUES ($1, 'Adversarial', 'note', '', true, $2)"#,
        id,
        alice
    )
    .execute(&pool)
    .await
    .unwrap();

    // When
    let deleted = template::delete(&pool, &alice, &id).await.unwrap();
    // Then
    assert!(
        !deleted,
        "is_built_in = true must block deletion even when user_id matches"
    );
    let still_there = template::find_by_id(&pool, &id).await.unwrap();
    assert!(still_there.is_some());
}

/// Sanity: deleting one's own non-built-in template succeeds.
#[sqlx::test]
async fn delete_removes_owned_non_builtin_row(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let created = template::create(
        &pool,
        &uid,
        NewTemplate {
            name: "Disposable".into(),
            description: None,
            template_type: "note".into(),
            content: "".into(),
            match_rules: None,
            field_mappings: None,
            priority: None,
            tags: None,
        },
    )
    .await
    .unwrap();

    // When
    let deleted = template::delete(&pool, &uid, &created.id).await.unwrap();
    // Then
    assert!(deleted);
    assert!(
        template::find_by_id(&pool, &created.id)
            .await
            .unwrap()
            .is_none()
    );
}

/// `find_by_id` is unscoped by design — matching Node's `getTemplateById`,
/// it returns any row by id regardless of caller, which is why the
/// mutation-time ownership guard has to live in `update`/`delete`'s own
/// `WHERE user_id = $2`, not here.
#[sqlx::test]
async fn find_by_id_is_unscoped_by_design(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let created = template::create(
        &pool,
        &alice,
        NewTemplate {
            name: "Alice's".into(),
            description: None,
            template_type: "note".into(),
            content: "".into(),
            match_rules: None,
            field_mappings: None,
            priority: None,
            tags: None,
        },
    )
    .await
    .unwrap();

    // When
    // No user_id parameter to pass at all -- this is the point.
    let found = template::find_by_id(&pool, &created.id).await.unwrap();
    // Then
    assert!(found.is_some());
}
