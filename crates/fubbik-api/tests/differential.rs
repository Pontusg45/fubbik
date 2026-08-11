//! Compares Node and Rust responses for the ported endpoints.
//!
//! Run with both stacks live:
//!   FUBBIK_NODE_URL=http://localhost:3000 \
//!   FUBBIK_RUST_URL=http://localhost:3100 \
//!   cargo test -p fubbik-api --test differential -- --ignored --nocapture
//!
//! With the env vars unset (the default for `cargo test`), the live-comparison
//! test is `#[ignore]`d and never contacts the network. The `normalise_*` unit
//! tests below always run and prove the comparison logic itself can both pass
//! on equivalent payloads and fail on divergent ones — see the module docs on
//! each test for what they establish.
//!
//! ## Deliberate divergences from Node
//!
//! These five behaviours are intentional decisions made while porting to
//! Rust, not defects to "fix" back toward Node if this harness (or a future
//! reader) notices them:
//!
//! 1. Chunk `create`/`update` reject a blank title; Node rejects on neither.
//!    *(Phase 1.)*
//! 2. `POST /api/tags/merge` returns 404 for an unknown/non-owned tag id;
//!    Node returns 500.
//! 3. `PATCH /api/spaces/{id}` leaves omitted fields untouched; Node clears
//!    `remoteUrl`/`localPaths` whenever the body omits them.
//! 4. `POST /api/spaces/{id}/reset` has an upfront ownership guard Node
//!    lacks.
//! 5. `POST /api/connections` returns 400 for an invalid `relation`; Node
//!    surfaces a raw 500.
//! 6. `GET /api/spaces`, `GET /api/tags`, and `GET /api/tag-types` are
//!    compared as unordered sets, not sequences (see
//!    `is_order_undefined_in_node` below). This is not really a divergence
//!    at all: Node's implementations of these three reads
//!    (`packages/db/src/repository/space.ts:74-76`,
//!    `packages/db/src/repository/tag-new.ts`'s `getTagsForUser`,
//!    `packages/db/src/repository/tag-type.ts:14`) have no `.orderBy(..)`
//!    anywhere in the repository or service layer, so their row order is
//!    *undefined*, not merely unspecified-but-stable. There is no defined
//!    Node behaviour to diverge from — asserting sequence equality would
//!    be asserting a property Node itself never promises, and would flap
//!    on whatever plan Node's query planner picks on a given run.
//!
//! Divergences 2-5 are all on **mutating** endpoints (POST/PATCH), and this
//! harness currently only diffs GETs — so it will NOT catch any of them
//! today. A clean run of this harness is evidence of GET parity only; it is
//! not evidence that these mutating-endpoint behaviours match Node (they
//! deliberately don't, per above).
//!
//! ## Phase 2b coverage (notifications/favorites/workspaces/settings/
//! ## collections/activity)
//!
//! Every `GET` these six domains' routers actually expose (read straight
//! from `crates/fubbik-api/src/{notifications,favorites,workspaces,settings,
//! collections,activity}/routes.rs`, not assumed) is now diffed:
//! `/api/notifications`, `/api/notifications/count`, `/api/favorites`,
//! `/api/workspaces`, `/api/workspaces/{id}`, `/api/settings/user`,
//! `/api/settings/codebase`, `/api/settings/instance`,
//! `/api/settings/features`, `/api/collections`,
//! `/api/collections/{id}/chunks`, `/api/activity`. There is **no**
//! `GET /api/collections/{id}` in either stack — only `PATCH`/`DELETE` take
//! a bare `{id}`, confirmed against `packages/api/src/collections/routes.ts`
//! — so it is not tested as a `GET`.
//!
//! `/api/settings/instance` and `/api/settings/features` are compared with
//! **no session cookie**, matching their deliberately unauthenticated status
//! in both stacks (`settings::routes::get_instance_settings` /
//! `get_feature_flags` doc comments) — every other new endpoint here is
//! session-scoped and compared with one.
//!
//! `/api/workspaces` joins the unordered set below (Node's `listWorkspaces`
//! has no `.orderBy`). `/api/workspaces/{id}` is a top-level *object*, so it
//! isn't eligible for the top-level-array multiset comparison at all — but
//! its nested `spaces` field has the exact same problem one level down:
//! Node's `getSpacesForWorkspace` (`packages/db/src/repository/
//! workspace.ts:80-91`) also has no `.orderBy`. `assert_same_with_unordered_field`
//! below handles that by sorting just that one nested array on both sides
//! before comparing, leaving the rest of the object (including its
//! top-level field order, which `serde_json::Value` equality already
//! ignores) compared exactly.
//!
//! `/api/notifications` (Node: `notification.ts:16`,
//! `.orderBy(desc(notification.createdAt))`), `/api/favorites` (Node:
//! `favorite.ts:7`, `.orderBy(asc(userFavorite.order))`), `/api/collections`
//! (Node: `collection.ts:9`, `.orderBy(asc(collection.name))`),
//! `/api/collections/{id}/chunks` (delegates to the same ordered chunk-list
//! query `/api/chunks` already uses), and `/api/activity` (Node:
//! `activity.ts:25`, `.orderBy(desc(activityLog.createdAt))`) all have an
//! explicit Node `ORDER BY` and stay sequence-compared. None of these are a
//! *total* order (no `id` tiebreaker on the Node side), so — per the
//! `/api/chunks?search=convention` precedent above — a live run tying on the
//! sort column can legitimately fail here; that is signal to report, not a
//! reason to move any of them into the unordered set.
//!
//! `/api/notifications/count`, `/api/settings/user`, `/api/settings/
//! codebase`, `/api/settings/instance`, and `/api/settings/features` are all
//! bare objects (a count, or a `{key: value}` settings map/computed flags
//! struct), not lists — "order" doesn't apply, and object key order is
//! already ignored by `serde_json::Value` equality (this crate does not
//! enable `preserve_order`), so they stay on the default sequence-compare
//! path with no special handling needed.
//!
//! ## A real bug the harness found, and how it was fixed
//!
//! A live run against Node found that `/api/tags` and
//! `/api/chunks?search=convention` returned the same row sets in different
//! orders. Root cause: several rows tied on the sort column
//! (`created_at`), and neither stack broke ties deterministically, so the
//! returned order was a query-plan artifact — internally stable within a
//! process, but not guaranteed to agree between two independently-planned
//! queries, and in principle not even guaranteed to agree between two
//! calls to the *same* query if a `LIMIT`/`OFFSET` walk crosses a plan
//! change. Every Rust list query (`chunk`, `tag`, `tag_type`, `space`, and
//! the chunk `applies_to`/`file_ref` sub-resource lists) now appends an
//! `id ASC` tiebreaker for exactly this reason — see `chunk::list`'s doc
//! comment in `crates/fubbik-db/src/repo/chunk.rs` for the full
//! explanation and `crates/fubbik-db/tests/{chunk,tag,tag_type,space}.rs`
//! for tests proving ties are broken deterministically. That closes the
//! gap for `/api/chunks?search=convention`, where Node *does* specify an
//! order (just not a total one). It cannot close the gap for `/api/tags`,
//! because Node's tag list has no order to make total in the first place
//! — hence divergence #6 above and the unordered comparison below.
//!
//! ## Divergence #11, resolved: the chunk-list limit clamp
//!
//! Rust used to clamp `GET /api/chunks?limit=` to `.clamp(1, 500)`
//! (`chunks::dto::ListChunksQuery::into_params`); Node clamps to 100
//! (`Math.min(Number(query.limit ?? 50), 100)`,
//! `packages/api/src/chunks/service.ts:50`). This survived three phases
//! unnoticed because the only limit case here, `?limit=5`, sits below BOTH
//! caps and can never observe a difference between them. Rust now also
//! clamps to 100, and `chunk_list_limit_is_clamped_identically_above_both_caps`
//! below adds a case *above* both caps (`?limit=200`) specifically so this
//! class of bug can't hide the same way twice.
//!
//! ## Phase 2c coverage (plans/search/staleness)
//!
//! Every `GET` these three domains' routers actually expose (read from
//! `crates/fubbik-api/src/{plans,search,staleness}/routes.rs`) is now
//! diffed. `connections::routes` has no `GET` at all (only `POST`/`DELETE`),
//! so there is nothing to add there.
//!
//! **Plans** (`crates/fubbik-api/src/plans/routes.rs`):
//! - `/api/plans` — Node: `packages/db/src/repository/plan.ts:113`,
//!   `.orderBy(asc(plan.createdAt))` on `listPlansWithRollups`. Ordered,
//!   sequence-compared.
//! - `/api/plans/{id}` — an envelope of five fields with genuinely mixed
//!   ordering, so it gets its own comparator, `assert_plan_detail_same`,
//!   rather than joining the plain sequence/multiset split:
//!   - `plan`: a single object, not a list.
//!   - `requirements`: Node `plan.ts:296`, `.orderBy(asc(planRequirement.order))`.
//!     Ordered.
//!   - `analyze`: Node `plan.ts:342`,
//!     `.orderBy(asc(planAnalyzeItem.kind), asc(planAnalyzeItem.order))`.
//!     Ordered (each of the five kind-buckets internally).
//!   - `tasks` (top level): Node `plan.ts:398`, `.orderBy(asc(planTask.order))`.
//!     Ordered.
//!   - `tasks[].chunks` (nested per task): Node `plan.ts:449-481`,
//!     `listTaskChunks`/`listTaskChunksWithTitles` — **no** `.orderBy`
//!     anywhere in either. Unordered; sorted in place before comparing.
//!   - `dependencies` (top level, flat): Node `plan.ts:503-516`,
//!     `listTaskDependencies` — **no** `.orderBy`. Unordered; sorted in
//!     place before comparing.
//! - `/api/plans/{id}/activity` — no longer always `[]`. Both stacks now write
//!   `activity_log` rows for plan create/update/delete/duplicate and for the
//!   three task-level mutations, so this compares real data. Node orders by
//!   `createdAt` desc and slices to 100; sequence-compared.
//! - `/api/plans/{id}/links` — Node: `plan.ts:566`,
//!   `.orderBy(asc(planExternalLink.order), asc(planExternalLink.createdAt))`.
//!   Ordered.
//! - `/api/plans/{id}/analyze` — same `listAnalyzeItems` query as the
//!   nested `analyze` field above (`plan.ts:342`). Ordered.
//! - `/api/plans/{id}/tasks/{taskId}/links` — Node: `plan.ts:590`,
//!   `.orderBy(asc(planTaskExternalLink.order), asc(planTaskExternalLink.createdAt))`.
//!   Ordered. Its `taskId` is fetched live from a real plan's first task
//!   (`first_task_id`), same reasoning as `first_id` elsewhere in this file.
//!
//! **Search** (`crates/fubbik-api/src/search/routes.rs`):
//! - `/api/search/parse` — no database call at all (pure parser). Trivially
//!   sequence-compared.
//! - `/api/search/autocomplete` — Node: `search/service.ts`'s `autocomplete`
//!   dispatches on `field` to `getTagsForUser` (`tag-new.ts`, no
//!   `.orderBy`, already divergence #6), `searchChunkTitles`
//!   (`chunk.ts:586-594`, no `.orderBy`), or `searchRequirementTitles`
//!   (`requirement.ts:292-300`, no `.orderBy`) — every reachable branch is
//!   unordered, so this path joins `is_order_undefined_in_node` outright
//!   rather than needing a per-`field` special case.
//! - `/api/search/saved` — Node: `packages/db/src/repository/
//!   saved-query.ts:14`, `.orderBy(desc(savedQuery.createdAt))`. Ordered.
//!
//! **Staleness** (`crates/fubbik-api/src/staleness/routes.rs`, mounted
//! under `/api/chunks/stale*`):
//! - `/api/chunks/stale` — Node: `packages/db/src/repository/
//!   staleness.ts:42`, `.orderBy(desc(chunkStaleness.detectedAt))`. Ordered.
//! - `/api/chunks/stale/count` — a **bare number** rendered as `text/plain`
//!   (the literal byte `0`, verified with `xxd`), NOT a `{count: N}` object.
//!   Phase 2b's `/api/notifications/count` DOES return an object, so reasoning
//!   by analogy gets this wrong; response shape is per-endpoint here. "order"
//!   doesn't apply — sequence-compared.
//!
//! (`getStaleFlagsForChunk`, `staleness.ts:61-74`, no `.orderBy`, feeds the
//! chunk-detail context resolver rather than any endpoint this harness
//! diffs — noted here only because the task brief calls it out by name.)

use serde_json::{Value, json};

/// Reads both live-stack base URLs, or panics.
///
/// These six `#[ignore]`d tests never execute their bodies under a plain
/// `cargo test --workspace` — Rust's default test harness skips `#[ignore]`d
/// tests entirely, so `urls()` panicking here has no effect on that path
/// (they still report as "ignored", not "failed"). It only runs when someone
/// explicitly requests `--ignored` (or `--include-ignored`), at which point
/// silently skipping with an `eprintln!` produced six meaningless passes —
/// a harness that reports success while doing nothing, which reads as
/// coverage it doesn't have. See the module doc's run instructions at the
/// top of this file for the two env vars this requires.
fn urls() -> (String, String) {
    let node = std::env::var("FUBBIK_NODE_URL").unwrap_or_else(|_| {
        panic!(
            "FUBBIK_NODE_URL not set: the live differential tests cannot run without both stacks"
        )
    });
    let rust = std::env::var("FUBBIK_RUST_URL").unwrap_or_else(|_| {
        panic!(
            "FUBBIK_RUST_URL not set: the live differential tests cannot run without both stacks"
        )
    });
    (node, rust)
}

/// Removes values that legitimately differ between stacks: generated IDs and
/// timestamps. Comparing them would produce noise, not signal.
fn normalise(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in ["id", "createdAt", "updatedAt", "userId"] {
                map.remove(key);
            }
            for (_, v) in map.iter_mut() {
                normalise(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(normalise),
        _ => {}
    }
}

async fn fetch(base: &str, path: &str) -> (u16, Value) {
    let res = reqwest::get(format!("{base}{path}"))
        .await
        .expect("request succeeds");
    let status = res.status().as_u16();
    let body = res.json::<Value>().await.unwrap_or(Value::Null);
    (status, body)
}

/// GET paths whose Node implementation has no `ORDER BY` anywhere in the
/// read path (repository OR service layer) — see the module doc
/// "deliberate divergences" entry #6 for the citations. Their row order is
/// undefined in Node, not merely unspecified-but-stable, so this harness
/// compares them as an unordered set rather than a sequence.
///
/// `/api/workspaces` joins this set in Phase 2b: Node's `listWorkspaces`
/// (`packages/db/src/repository/workspace.ts:37`) has no `.orderBy` either
/// — see the module doc's "Phase 2b coverage" section.
///
/// `/api/search/autocomplete` joins this set in Phase 2c: every branch of
/// Node's `autocomplete` (`getTagsForUser`, `searchChunkTitles`,
/// `searchRequirementTitles`) has no `.orderBy` — see the module doc's
/// "Phase 2c coverage" section.
///
/// Every OTHER list endpoint this harness diffs (`/api/chunks*`,
/// `/api/notifications`, `/api/favorites`, `/api/collections`,
/// `/api/collections/{id}/chunks`, `/api/activity`, `/api/plans`,
/// `/api/plans/{id}/links`, `/api/plans/{id}/analyze`,
/// `/api/plans/{id}/tasks/{taskId}/links`, `/api/search/saved`,
/// `/api/chunks/stale`) DOES have an explicit `ORDER BY` in Node — for
/// those, sequence comparison stays in effect, because an ordering
/// regression there is real signal, not noise. Matched on the path with
/// any query string stripped, so e.g. `/api/tags?search=x` (should such a
/// variant ever be added here) would still be treated as unordered.
fn is_order_undefined_in_node(path: &str) -> bool {
    matches!(
        path.split('?').next().unwrap_or(path),
        "/api/spaces"
            | "/api/tags"
            | "/api/tag-types"
            | "/api/workspaces"
            | "/api/search/autocomplete"
    )
}

/// Sorts an unordered nested array field, in place, by each element's
/// canonical JSON string. Used only on `/api/workspaces/{id}`'s `spaces`
/// field (see the module doc's "Phase 2b coverage" section for why that
/// one nested array — unlike the rest of the object — has no defined Node
/// order). Not a general-purpose tool: it only looks one level deep, at a
/// named field directly on a top-level object, which is all this endpoint
/// needs.
fn sort_nested_array(value: &mut Value, field: &str) {
    if let Value::Object(map) = value
        && let Some(Value::Array(items)) = map.get_mut(field)
    {
        items.sort_by_key(canonical);
    }
}

/// Like `assert_same`, but for a top-level *object* response with exactly
/// one nested array field whose order is undefined in Node — sorts that
/// field on both sides before comparing so element order alone can't fail
/// the assertion, while every other field (including ones Node DOES order)
/// stays an exact match.
async fn assert_same_with_unordered_field(path: &str, field: &str) {
    let (node, rust) = urls();

    let (node_status, mut node_body) = fetch(&node, path).await;
    let (rust_status, mut rust_body) = fetch(&rust, path).await;

    assert_eq!(node_status, rust_status, "status mismatch for {path}");

    normalise(&mut node_body);
    normalise(&mut rust_body);
    sort_nested_array(&mut node_body, field);
    sort_nested_array(&mut rust_body, field);

    assert_eq!(
        node_body, rust_body,
        "body mismatch for {path} (after sorting the unordered `{field}` field)"
    );
}

/// Fetches a bare top-level array from `base`+`list_path` and returns the
/// first element's `id`, or `None` if the list is empty or not an array of
/// objects with a string `id`. Used to make path-parameterised endpoints
/// (`/api/workspaces/{id}`, `/api/collections/{id}/chunks`,
/// `/api/settings/codebase?codebaseId=`) meaningful against whatever data
/// happens to be loaded into the diff database — this harness has no fixed
/// seed of its own, it runs against a dump of the Node database (see
/// `scripts/differential.sh`). Fetching the id from `rust`'s own list
/// response (rather than a hardcoded literal) also means the id is
/// guaranteed to round-trip through Rust's own list endpoint first.
async fn first_id(base: &str, list_path: &str) -> Option<String> {
    let (_, body) = fetch(base, list_path).await;
    body.as_array()?
        .first()?
        .get("id")?
        .as_str()
        .map(String::from)
}

/// Fetches `GET {base}/api/plans/{plan_id}` and returns the first task's
/// `id` from the nested `tasks` array, or `None` if the plan has no tasks.
/// Used to make `/api/plans/{id}/tasks/{taskId}/links` meaningful against
/// whatever plan/task data the diff database happens to hold — same
/// reasoning as `first_id`, one level deeper.
async fn first_task_id(base: &str, plan_id: &str) -> Option<String> {
    let (_, body) = fetch(base, &format!("/api/plans/{plan_id}")).await;
    body.get("tasks")?
        .as_array()?
        .first()?
        .get("id")?
        .as_str()
        .map(String::from)
}

/// Canonicalises a JSON value into a string for multiset comparison.
/// `serde_json::Value::to_string` on an object always emits keys in
/// `BTreeMap` order — this crate does not enable serde_json's
/// `preserve_order` feature — so two structurally-identical objects
/// serialize identically regardless of which field order either backend
/// happened to emit them in.
fn canonical(value: &Value) -> String {
    value.to_string()
}

/// Order-insensitive body comparison: sorts each side's top-level array by
/// its elements' canonical JSON string and compares the sorted lists. Used
/// only for the paths `is_order_undefined_in_node` names.
fn assert_same_as_multiset(node_body: &Value, rust_body: &Value, path: &str) {
    let (Value::Array(node_items), Value::Array(rust_items)) = (node_body, rust_body) else {
        panic!(
            "expected array bodies for order-insensitive comparison of {path}, \
             got node={node_body:?} rust={rust_body:?}"
        );
    };
    let mut node_sorted: Vec<String> = node_items.iter().map(canonical).collect();
    let mut rust_sorted: Vec<String> = rust_items.iter().map(canonical).collect();
    node_sorted.sort();
    rust_sorted.sort();
    assert_eq!(
        node_sorted, rust_sorted,
        "body mismatch (compared as a set, ignoring order) for {path}"
    );
}

async fn assert_same(path: &str) {
    let (node, rust) = urls();

    let (node_status, mut node_body) = fetch(&node, path).await;
    let (rust_status, mut rust_body) = fetch(&rust, path).await;

    assert_eq!(node_status, rust_status, "status mismatch for {path}");

    normalise(&mut node_body);
    normalise(&mut rust_body);

    if is_order_undefined_in_node(path) {
        assert_same_as_multiset(&node_body, &rust_body, path);
    } else {
        assert_eq!(node_body, rust_body, "body mismatch for {path}");
    }
}

/// Sorts the two nested arrays of a `GET /api/plans/{id}` response that
/// Node leaves unordered — `dependencies` (top level, flat) and each
/// element of `tasks[].chunks` (nested per task) — in place, by canonical
/// JSON string. `plan`, `requirements`, `analyze`, and the top-level
/// `tasks` sequence itself are all left untouched because Node orders all
/// of them (see the module doc's "Phase 2c coverage" section for the
/// per-field citations).
fn sort_plan_detail_unordered_fields(value: &mut Value) {
    let Value::Object(map) = value else {
        return;
    };
    if let Some(Value::Array(deps)) = map.get_mut("dependencies") {
        deps.sort_by_key(canonical);
    }
    if let Some(Value::Array(tasks)) = map.get_mut("tasks") {
        for task in tasks.iter_mut() {
            if let Value::Object(task_map) = task
                && let Some(Value::Array(chunks)) = task_map.get_mut("chunks")
            {
                chunks.sort_by_key(canonical);
            }
        }
    }
}

/// Like `assert_same`, but for `GET /api/plans/{id}`, whose response mixes
/// ordered and unordered nested arrays within the same object (see the
/// module doc's "Phase 2c coverage" section) — too irregular a shape for
/// either `assert_same`'s single-path branch or
/// `assert_same_with_unordered_field`'s single-named-field sort.
async fn assert_plan_detail_same(path: &str) {
    let (node, rust) = urls();

    let (node_status, mut node_body) = fetch(&node, path).await;
    let (rust_status, mut rust_body) = fetch(&rust, path).await;

    assert_eq!(node_status, rust_status, "status mismatch for {path}");

    normalise(&mut node_body);
    normalise(&mut rust_body);
    sort_plan_detail_unordered_fields(&mut node_body);
    sort_plan_detail_unordered_fields(&mut rust_body);

    assert_eq!(
        node_body, rust_body,
        "body mismatch for {path} (after sorting `dependencies` and each task's `chunks`)"
    );
}

#[tokio::test]
#[ignore = "requires both stacks running"]
async fn chunk_endpoints_match() {
    for path in [
        "/api/chunks",
        "/api/chunks?type=note",
        "/api/chunks?limit=5",
        "/api/chunks?sort=alpha",
        "/api/chunks?search=convention",
    ] {
        assert_same(path).await;
    }
}

/// Divergence #11, resolved: the pre-existing `?limit=5` case above sits
/// below BOTH Node's cap (100) and Rust's old cap (500), so it could never
/// see the two stacks disagree — that blind spot is exactly how this
/// divergence survived three phases. `?limit=200` sits above both, so a
/// clamp mismatch is now visible: before the fix, Node returns at most 100
/// chunks and Rust returned up to 200, and this assertion fails; after the
/// fix, both return at most 100 and it passes. See the module doc's
/// "Divergence #11, resolved" section.
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn chunk_list_limit_is_clamped_identically_above_both_caps() {
    let (node, rust) = urls();

    let (_, node_body) = fetch(&node, "/api/chunks?limit=200").await;
    let (_, rust_body) = fetch(&rust, "/api/chunks?limit=200").await;

    assert_eq!(
        node_body["chunks"].as_array().unwrap().len(),
        rust_body["chunks"].as_array().unwrap().len(),
        "both stacks must clamp the chunk list to the same maximum"
    );
}

/// The four GET endpoints ported in this slice. Unlike the chunk endpoints
/// above, none of these return the `{chunks,total,limit,offset}` envelope —
/// `/api/spaces`, `/api/tags`, and `/api/tag-types` return bare arrays, and
/// `/api/stats` returns a bare object of counts. See the `normalise_*` unit
/// tests below proving `normalise` handles both shapes correctly.
///
/// `/api/spaces`, `/api/tags`, and `/api/tag-types` are compared as
/// unordered sets via `is_order_undefined_in_node`, not sequences — Node
/// has no `ORDER BY` for any of the three. `/api/stats` stays an exact
/// comparison: it is a single object, not a list, so "order" does not
/// apply, and its counts are exactly the signal this test exists to check.
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn spaces_tags_tag_types_stats_match() {
    for path in ["/api/spaces", "/api/tags", "/api/tag-types", "/api/stats"] {
        assert_same(path).await;
    }
}

/// The Phase 2b `GET` endpoints that need no path/query param to be
/// meaningful — plain lists, a count, and the settings/feature-flag
/// objects. See the module doc's "Phase 2b coverage" section for the
/// per-path ordering citations; every one of these stays sequence-compared
/// (`assert_same` only takes the unordered branch for paths named in
/// `is_order_undefined_in_node`, and none of these are).
///
/// `/api/settings/instance` and `/api/settings/features` are deliberately
/// unauthenticated in both stacks — `fetch`/`assert_same` send no session
/// cookie at all (matching every other path in this file, which relies on
/// `FUBBIK_IMPLICIT_DEV_SESSION` on both servers rather than a real cookie),
/// so this is already the correct "no session" comparison for those two,
/// not a special case that needs different plumbing.
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn notifications_favorites_settings_activity_match() {
    for path in [
        "/api/notifications",
        "/api/notifications/count",
        "/api/favorites",
        "/api/settings/user",
        "/api/settings/instance",
        "/api/settings/features",
        "/api/activity",
    ] {
        assert_same(path).await;
    }
}

/// `/api/workspaces` (unordered — Node's `listWorkspaces` has no
/// `.orderBy`, joining `/api/spaces`/`/api/tags`/`/api/tag-types` in
/// `is_order_undefined_in_node`) plus `/api/workspaces/{id}`, whose id is
/// fetched from the diff database's own `/api/workspaces` list rather than
/// hardcoded — there is no fixed seed this harness controls (see
/// `first_id`'s doc comment). The detail endpoint's nested `spaces` field
/// is unordered for the same reason the list is (`getSpacesForWorkspace`
/// has no `.orderBy` either); the rest of the object stays exact via
/// `assert_same_with_unordered_field`.
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn workspaces_list_and_detail_match() {
    assert_same("/api/workspaces").await;

    let (_, rust) = urls();
    let Some(id) = first_id(&rust, "/api/workspaces").await else {
        eprintln!("skipping /api/workspaces/{{id}}: no workspaces in the diff database");
        return;
    };
    assert_same_with_unordered_field(&format!("/api/workspaces/{id}"), "spaces").await;
}

/// `/api/collections` (Node orders by `name`, sequence-compared) plus
/// `/api/collections/{id}/chunks`, whose id is likewise fetched live rather
/// than hardcoded. The chunks endpoint delegates to the same ordered
/// chunk-list query `/api/chunks` already uses (see the module doc), so it
/// is compared exactly like the `chunk_endpoints_match` paths above, not
/// through any unordered path.
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn collections_list_and_chunks_match() {
    assert_same("/api/collections").await;

    let (_, rust) = urls();
    let Some(id) = first_id(&rust, "/api/collections").await else {
        eprintln!("skipping /api/collections/{{id}}/chunks: no collections in the diff database");
        return;
    };
    assert_same(&format!("/api/collections/{id}/chunks")).await;
}

/// `/api/settings/codebase?codebaseId=` needs an existing space id to be
/// meaningful (an unknown one 404s on both stacks per `settings::service`'s
/// doc comments) — fetched live from `/api/spaces`, same reasoning as the
/// workspace/collection ids above. A bare `{key: value}` object, so it
/// stays on the default sequence-compare path (object key order is not
/// significant either way — see the module doc).
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn settings_codebase_matches() {
    let (_, rust) = urls();
    let Some(space_id) = first_id(&rust, "/api/spaces").await else {
        eprintln!("skipping /api/settings/codebase: no spaces in the diff database");
        return;
    };
    assert_same(&format!("/api/settings/codebase?codebaseId={space_id}")).await;
}

/// `/api/plans` (Node orders by `createdAt`, sequence-compared) plus the
/// plan-scoped GETs that need no more than a plan id to be meaningful:
/// `/api/plans/{id}/activity` (always `[]` on both stacks), `/api/plans/{id}/links`,
/// and `/api/plans/{id}/analyze` — all three ordered in Node, per the
/// module doc's "Phase 2c coverage" section. `/api/plans/{id}` itself
/// (mixed ordering) and `/api/plans/{id}/tasks/{taskId}/links` (needs a
/// task id, not just a plan id) get their own tests below.
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn plans_list_and_simple_details_match() {
    assert_same("/api/plans").await;

    let (_, rust) = urls();
    let Some(id) = first_id(&rust, "/api/plans").await else {
        eprintln!("skipping /api/plans/{{id}}/*: no plans in the diff database");
        return;
    };
    assert_same(&format!("/api/plans/{id}/activity")).await;
    assert_same(&format!("/api/plans/{id}/links")).await;
    assert_same(&format!("/api/plans/{id}/analyze")).await;
}

/// `/api/plans/{id}` mixes ordered and unordered nested arrays — see
/// `assert_plan_detail_same`'s doc comment and the module doc's "Phase 2c
/// coverage" section for the per-field citations.
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn plan_detail_matches() {
    let (_, rust) = urls();
    let Some(id) = first_id(&rust, "/api/plans").await else {
        eprintln!("skipping /api/plans/{{id}}: no plans in the diff database");
        return;
    };
    assert_plan_detail_same(&format!("/api/plans/{id}")).await;
}

/// `/api/plans/{id}/tasks/{taskId}/links` (Node: ordered, see the module
/// doc) needs both a plan id and one of its task ids — both fetched live
/// via `first_id`/`first_task_id` rather than hardcoded, same reasoning as
/// every other path-parameterised endpoint in this file.
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn plan_task_links_match() {
    let (_, rust) = urls();
    let Some(plan_id) = first_id(&rust, "/api/plans").await else {
        eprintln!(
            "skipping /api/plans/{{id}}/tasks/{{taskId}}/links: no plans in the diff database"
        );
        return;
    };
    let Some(task_id) = first_task_id(&rust, &plan_id).await else {
        eprintln!("skipping /api/plans/{{id}}/tasks/{{taskId}}/links: plan {plan_id} has no tasks");
        return;
    };
    assert_same(&format!("/api/plans/{plan_id}/tasks/{task_id}/links")).await;
}

/// `/api/search/parse` (no DB call at all) and `/api/search/saved` (Node
/// orders by `createdAt`, sequence-compared) — see the module doc's "Phase
/// 2c coverage" section. `/api/search/autocomplete` gets its own test below
/// since every valid `field` value is unordered in Node.
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn search_parse_and_saved_match() {
    assert_same("/api/search/parse?q=type:note").await;
    assert_same("/api/search/saved").await;
}

/// `/api/search/autocomplete` — unordered for every valid `field`
/// (`tag`/`chunk`/`requirement`), per the module doc's "Phase 2c coverage"
/// section and `is_order_undefined_in_node`'s doc comment. An unrecognised
/// `field` falling through to Node's `return [];` is trivially order-safe
/// too, so it doesn't need its own case here.
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn search_autocomplete_matches() {
    for path in [
        "/api/search/autocomplete?field=tag&prefix=a",
        "/api/search/autocomplete?field=chunk&prefix=a",
        "/api/search/autocomplete?field=requirement&prefix=a",
    ] {
        assert_same(path).await;
    }
}

/// `/api/chunks/stale` (Node orders by `detectedAt`, sequence-compared) and
/// `/api/chunks/stale/count` (a bare number as `text/plain`, not an object and
/// not a list) — see the module doc's "Phase 2c coverage" section.
#[tokio::test]
#[ignore = "requires both stacks running"]
async fn staleness_endpoints_match() {
    assert_same("/api/chunks/stale").await;
    assert_same("/api/chunks/stale/count").await;
}

/// `normalise` must strip exactly the four volatile keys (at any nesting
/// depth) so that two payloads differing ONLY in generated IDs/timestamps
/// compare equal. If this test failed, the harness would report false
/// positives on every real run — legitimate stack-to-stack noise would look
/// like a parity bug.
#[test]
fn normalise_treats_id_and_timestamp_differences_as_equal() {
    let mut node = json!({
        "id": "node-generated-uuid",
        "title": "Naming convention",
        "content": "kebab-case",
        "type": "note",
        "createdAt": "2026-01-01T00:00:00Z",
        "updatedAt": "2026-01-02T00:00:00Z",
        "userId": "node-user-1",
        "connections": [
            { "id": "conn-1", "targetId": "chunk-a", "relation": "related_to" }
        ]
    });
    let mut rust = json!({
        "id": "rust-generated-uuid",
        "title": "Naming convention",
        "content": "kebab-case",
        "type": "note",
        "createdAt": "2026-06-15T09:30:00Z",
        "updatedAt": "2026-06-15T09:30:00Z",
        "userId": "rust-user-9",
        "connections": [
            { "id": "conn-9", "targetId": "chunk-a", "relation": "related_to" }
        ]
    });

    normalise(&mut node);
    normalise(&mut rust);

    assert_eq!(
        node, rust,
        "payloads differing only in id/createdAt/updatedAt/userId must normalise equal"
    );
}

/// The harness must not be a rubber stamp: two payloads that differ in a
/// MEANINGFUL field (here, `content`) must still compare unequal after
/// normalisation. This is the single most important property of the
/// harness — without it, a real parity bug between Node and Rust would
/// silently pass.
#[test]
fn normalise_still_detects_a_real_content_difference() {
    let mut node = json!({
        "id": "same-id",
        "title": "Naming convention",
        "content": "kebab-case",
        "createdAt": "2026-01-01T00:00:00Z",
        "updatedAt": "2026-01-01T00:00:00Z",
        "userId": "u1"
    });
    let mut rust = json!({
        "id": "same-id",
        "title": "Naming convention",
        "content": "snake_case",
        "createdAt": "2026-01-01T00:00:00Z",
        "updatedAt": "2026-01-01T00:00:00Z",
        "userId": "u1"
    });

    normalise(&mut node);
    normalise(&mut rust);

    assert_ne!(
        node, rust,
        "a real difference in `content` must survive normalisation"
    );
}

/// Guards against over-broad normalisation: nested objects that happen to
/// contain a key named `id`/`userId`/etc. at depth are still stripped (by
/// design, `normalise` is unconditional on key name, not path), but fields
/// with DIFFERENT names — even ones that sound similar, like `authorId`
/// versus `userId` — must never be silently dropped. This test pins down
/// that `authorId` differences are NOT masked.
#[test]
fn normalise_does_not_mask_fields_with_different_names() {
    let mut node = json!({
        "id": "x",
        "authorId": "user-a"
    });
    let mut rust = json!({
        "id": "x",
        "authorId": "user-b"
    });

    normalise(&mut node);
    normalise(&mut rust);

    assert_ne!(
        node, rust,
        "authorId is not one of the stripped keys and must still be compared"
    );
}

/// `/api/spaces`, `/api/tags`, and `/api/tag-types` return bare top-level
/// arrays, not the `{chunks,total,...}` envelope the chunk endpoints use.
/// `normalise`'s `Value::Array` arm must recurse into each element so the
/// same id/timestamp/userId stripping happens as for an object response —
/// otherwise every one of these three new endpoints would report false
/// positives on every real run, since every row carries its own `id` and
/// (for spaces) `createdAt`/`updatedAt`.
#[test]
fn normalise_recurses_into_a_top_level_array() {
    let mut node = json!([
        { "id": "node-1", "name": "fubbik", "kind": "code", "createdAt": "2026-01-01T00:00:00Z" },
        { "id": "node-2", "name": "other", "kind": "wiki", "createdAt": "2026-01-02T00:00:00Z" }
    ]);
    let mut rust = json!([
        { "id": "rust-1", "name": "fubbik", "kind": "code", "createdAt": "2026-06-15T09:30:00Z" },
        { "id": "rust-2", "name": "other", "kind": "wiki", "createdAt": "2026-06-15T09:31:00Z" }
    ]);

    normalise(&mut node);
    normalise(&mut rust);

    assert_eq!(
        node, rust,
        "a top-level array response must normalise element-by-element, same as an object"
    );
}

/// `/api/stats` returns a bare object of counts (`chunks`, `connections`,
/// `tags`) — no `id`, no timestamps, nothing `normalise` strips. Those
/// counts are exactly the signal the harness exists to check: the
/// `fubbik_diff` database is seeded from a dump of the Node database, so if
/// the two stacks count rows differently, that is a real Rust bug, not
/// noise. This test pins down that `normalise` does NOT touch numeric
/// fields — two stats objects differing only in `chunks` must still compare
/// UNEQUAL after normalisation, or a real counting bug would be silently
/// masked.
#[test]
fn normalise_does_not_mask_a_stats_count_difference() {
    let mut node = json!({ "chunks": 24, "connections": 36, "tags": 40 });
    let mut rust = json!({ "chunks": 23, "connections": 36, "tags": 40 });

    normalise(&mut node);
    normalise(&mut rust);

    assert_ne!(
        node, rust,
        "a real difference in a stats count must survive normalisation"
    );
}

/// Exactly the four endpoints named in "deliberate divergences" #6, plus
/// Phase 2b's `/api/workspaces` and Phase 2c's `/api/search/autocomplete`,
/// must be treated as unordered — not the chunk/notifications/favorites/
/// collections/activity/plans/search-saved/staleness endpoints, which DO
/// have a Node `ORDER BY` and must stay sequence-compared, and not
/// `/api/stats` or `/api/notifications/count` (single objects rather than
/// lists), nor `/api/chunks/stale/count` (a bare number as `text/plain`).
#[test]
fn is_order_undefined_in_node_covers_exactly_the_five_unordered_lists() {
    for path in [
        "/api/spaces",
        "/api/tags",
        "/api/tag-types",
        "/api/workspaces",
        "/api/search/autocomplete",
        "/api/search/autocomplete?field=chunk&prefix=a",
    ] {
        assert!(
            is_order_undefined_in_node(path),
            "{path} must be compared unordered: Node has no ORDER BY for it"
        );
    }

    for path in [
        "/api/chunks",
        "/api/chunks?type=note",
        "/api/chunks?limit=5",
        "/api/chunks?sort=alpha",
        "/api/chunks?search=convention",
        "/api/stats",
        "/api/notifications",
        "/api/notifications/count",
        "/api/favorites",
        "/api/collections",
        "/api/activity",
        "/api/settings/user",
        "/api/settings/instance",
        "/api/settings/features",
        "/api/plans",
        "/api/plans/abc/links",
        "/api/plans/abc/analyze",
        "/api/plans/abc/tasks/def/links",
        "/api/search/parse?q=type:note",
        "/api/search/saved",
        "/api/chunks/stale",
        "/api/chunks/stale/count",
    ] {
        assert!(
            !is_order_undefined_in_node(path),
            "{path} must stay sequence-compared: it either has a Node ORDER BY \
             or is not a list at all"
        );
    }
}

/// `sort_nested_array` must reorder only the named field's array elements,
/// leaving the rest of the object (and non-matching fields) untouched — the
/// property `assert_same_with_unordered_field` relies on to compare
/// `/api/workspaces/{id}`'s `spaces` field as an unordered set while every
/// other field stays an exact match.
#[test]
fn sort_nested_array_reorders_only_the_named_field() {
    let mut value = json!({
        "id": "w1",
        "name": "Frontend + backend",
        "spaces": [
            { "name": "gamma", "kind": "code" },
            { "name": "alpha", "kind": "code" }
        ]
    });

    sort_nested_array(&mut value, "spaces");

    assert_eq!(
        value,
        json!({
            "id": "w1",
            "name": "Frontend + backend",
            "spaces": [
                { "name": "alpha", "kind": "code" },
                { "name": "gamma", "kind": "code" }
            ]
        })
    );
}

/// A genuinely different `spaces` set (not just reordered) must still fail
/// after sorting — pins down that `assert_same_with_unordered_field`
/// doesn't accidentally become a rubber stamp for the one field it treats
/// specially.
#[test]
fn sort_nested_array_does_not_mask_a_real_difference() {
    let mut node = json!({ "spaces": [{ "name": "alpha" }, { "name": "beta" }] });
    let mut rust = json!({ "spaces": [{ "name": "alpha" }, { "name": "delta" }] });

    sort_nested_array(&mut node, "spaces");
    sort_nested_array(&mut rust, "spaces");

    assert_ne!(
        node, rust,
        "a genuinely different spaces set must survive sorting and still compare unequal"
    );
}

/// `sort_plan_detail_unordered_fields` must ignore pure reordering of the
/// two fields Node leaves unordered — the top-level `dependencies` array
/// and each task's nested `chunks` array — while every ordered field
/// (`tasks` itself, and each task's other fields) stays untouched.
#[test]
fn sort_plan_detail_unordered_fields_ignores_pure_reordering() {
    let mut node = json!({
        "plan": { "id": "p1" },
        "requirements": [],
        "analyze": {},
        "tasks": [
            { "id": "t1", "order": 0, "chunks": [{ "chunkId": "c1" }, { "chunkId": "c2" }] },
            { "id": "t2", "order": 1, "chunks": [] }
        ],
        "dependencies": [
            { "taskId": "t2", "dependsOnTaskId": "t1" },
            { "taskId": "t3", "dependsOnTaskId": "t1" }
        ]
    });
    let mut rust = json!({
        "plan": { "id": "p1" },
        "requirements": [],
        "analyze": {},
        "tasks": [
            { "id": "t1", "order": 0, "chunks": [{ "chunkId": "c2" }, { "chunkId": "c1" }] },
            { "id": "t2", "order": 1, "chunks": [] }
        ],
        "dependencies": [
            { "taskId": "t3", "dependsOnTaskId": "t1" },
            { "taskId": "t2", "dependsOnTaskId": "t1" }
        ]
    });

    sort_plan_detail_unordered_fields(&mut node);
    sort_plan_detail_unordered_fields(&mut rust);

    assert_eq!(
        node, rust,
        "reordering only `dependencies` and per-task `chunks` must not fail the comparison"
    );
}

/// The plan-detail comparator must not be a rubber stamp: a genuinely
/// different `chunks` set on one task, or a genuinely different top-level
/// `tasks` order (which Node DOES define — `plan.ts:398`,
/// `.orderBy(asc(planTask.order))`), must still compare unequal after
/// sorting.
#[test]
fn sort_plan_detail_unordered_fields_does_not_mask_real_differences() {
    let mut node = json!({
        "tasks": [{ "id": "t1", "chunks": [{ "chunkId": "c1" }] }],
        "dependencies": []
    });
    let mut different_chunks = json!({
        "tasks": [{ "id": "t1", "chunks": [{ "chunkId": "c9" }] }],
        "dependencies": []
    });
    sort_plan_detail_unordered_fields(&mut node);
    sort_plan_detail_unordered_fields(&mut different_chunks);
    assert_ne!(
        node, different_chunks,
        "a genuinely different per-task chunks set must survive sorting and still differ"
    );

    let mut task_order_a = json!({ "tasks": [{ "id": "t1" }, { "id": "t2" }], "dependencies": [] });
    let mut task_order_b = json!({ "tasks": [{ "id": "t2" }, { "id": "t1" }], "dependencies": [] });
    sort_plan_detail_unordered_fields(&mut task_order_a);
    sort_plan_detail_unordered_fields(&mut task_order_b);
    assert_ne!(
        task_order_a, task_order_b,
        "the top-level `tasks` sequence is ordered in Node and must NOT be sorted \
         away by the plan-detail comparator"
    );
}

/// The core property `assert_same_as_multiset` exists for: the live run
/// found `/api/tags` returning the SAME rows in a DIFFERENT order between
/// Node and Rust, which is not a bug (see #6) but would fail a naive
/// sequence comparison. This pins down that reordering alone does not fail
/// the multiset comparison.
#[test]
fn multiset_comparison_ignores_pure_reordering() {
    let node = json!([
        { "name": "alpha" },
        { "name": "beta" },
        { "name": "gamma" }
    ]);
    let rust = json!([
        { "name": "gamma" },
        { "name": "alpha" },
        { "name": "beta" }
    ]);

    assert_same_as_multiset(&node, &rust, "/api/tags");
}

/// The multiset comparison must not be a rubber stamp either: a genuinely
/// different row set (here, `beta` replaced by `delta`) must still fail,
/// even though both sides have the same length. Without this, a real
/// parity bug on one of the unordered endpoints would silently pass.
#[test]
#[should_panic(expected = "body mismatch (compared as a set")]
fn multiset_comparison_still_detects_a_different_row_set() {
    let node = json!([{ "name": "alpha" }, { "name": "beta" }]);
    let rust = json!([{ "name": "alpha" }, { "name": "delta" }]);

    assert_same_as_multiset(&node, &rust, "/api/tags");
}
