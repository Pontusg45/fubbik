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

use serde_json::{Value, json};

fn urls() -> Option<(String, String)> {
    Some((
        std::env::var("FUBBIK_NODE_URL").ok()?,
        std::env::var("FUBBIK_RUST_URL").ok()?,
    ))
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
/// Every OTHER list endpoint this harness diffs (`/api/chunks*`) DOES have
/// an explicit `ORDER BY` in Node — for those, sequence comparison stays
/// in effect, because an ordering regression there is real signal, not
/// noise. Matched on the path with any query string stripped, so e.g.
/// `/api/tags?search=x` (should such a variant ever be added here) would
/// still be treated as unordered.
fn is_order_undefined_in_node(path: &str) -> bool {
    matches!(
        path.split('?').next().unwrap_or(path),
        "/api/spaces" | "/api/tags" | "/api/tag-types"
    )
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
    let Some((node, rust)) = urls() else {
        eprintln!("skipping {path}: FUBBIK_NODE_URL / FUBBIK_RUST_URL not set");
        return;
    };

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

/// Exactly the three endpoints named in "deliberate divergences" #6 must
/// be treated as unordered — not the chunk endpoints, which DO have a
/// Node `ORDER BY` and must stay sequence-compared, and not `/api/stats`,
/// which is a single object rather than a list at all.
#[test]
fn is_order_undefined_in_node_covers_exactly_spaces_tags_and_tag_types() {
    for path in ["/api/spaces", "/api/tags", "/api/tag-types"] {
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
    ] {
        assert!(
            !is_order_undefined_in_node(path),
            "{path} must stay sequence-compared: it either has a Node ORDER BY \
             (chunks) or is not a list at all (stats)"
        );
    }
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
