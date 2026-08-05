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
//!
//! Divergences 2-5 are all on **mutating** endpoints (POST/PATCH), and this
//! harness currently only diffs GETs — so it will NOT catch any of them
//! today. A clean run of this harness is evidence of GET parity only; it is
//! not evidence that these mutating-endpoint behaviours match Node (they
//! deliberately don't, per above).

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
    assert_eq!(node_body, rust_body, "body mismatch for {path}");
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
