//! Guards against OpenAPI schema-name collisions.
//!
//! utoipa registers every `ToSchema` type in one flat namespace
//! (`#/components/schemas/<name>`), so two types sharing a Rust identifier in
//! different modules silently overwrite each other. Whichever loses has the
//! *other* type's shape published for its endpoints — the generated TypeScript
//! client then rejects valid calls, or accepts invalid ones.
//!
//! This has already shipped twice: `Position` (saved-graph `{x,y}` published as
//! vocabulary's `{start,end}`) and `BulkActionBody` (proposals' bulk body
//! published as requirements'). Both were caught only because a web call site
//! happened to fail to compile.
//!
//! The collision is undetectable in the finished spec — two types collapse into
//! one schema, which looks exactly like one type. So this scans the *sources*
//! for `ToSchema` derives and the name each will register under, which is the
//! only place the duplication is still visible.
//!
//! Duplicates are allowed only where every definition is structurally
//! identical, so whichever wins is still correct. Those are listed in
//! `ALLOWED_DUPLICATES`. Anything else is a bug: give one of the colliding
//! types `#[schema(as = SomeDistinctName)]`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Names that legitimately appear on more than one type because every
/// definition is byte-identical. Verified by reading them, not assumed.
const ALLOWED_DUPLICATES: &[&str] = &[
    "MessageResponse", // `{ message: String }`, in 15 domains
    "Origin",          // `Human | Ai`, in requirements and connections
    "ReviewStatus",    // `Draft | Reviewed | Approved`, in requirements and tags
];

#[test]
fn no_unexpected_openapi_schema_name_collisions() {
    let mut by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in rust_sources() {
        let src = std::fs::read_to_string(&file).expect("source file is readable");
        for (name, ident) in schema_names_in(&src) {
            by_name
                .entry(name)
                .or_default()
                .push(format!("{}::{ident}", file.display()));
        }
    }

    assert!(
        by_name.len() > 100,
        "only found {} ToSchema types — the source scan is broken, not the code",
        by_name.len()
    );

    let mut unexpected: Vec<String> = Vec::new();
    for (name, locations) in &by_name {
        if locations.len() > 1 && !ALLOWED_DUPLICATES.contains(&name.as_str()) {
            unexpected.push(format!(
                "  `{name}` is defined by:\n    {}",
                locations.join("\n    ")
            ));
        }
    }

    assert!(
        unexpected.is_empty(),
        "OpenAPI schema-name collision — these types share a name, so utoipa will \
         publish only one of them and the others' endpoints get the wrong body/response \
         schema in openapi.json (and therefore in the generated web client):\n\n{}\n\n\
         Fix: add `#[schema(as = DistinctName)]` to one of them. If they are structurally \
         identical and the collision is harmless, add the name to ALLOWED_DUPLICATES in \
         this test with a comment saying why.",
        unexpected.join("\n")
    );

    // Keep the allowlist honest: a stale entry would mask a future real collision.
    for name in ALLOWED_DUPLICATES {
        let count = by_name.get(*name).map_or(0, |v| v.len());
        assert!(
            count > 1,
            "`{name}` is on ALLOWED_DUPLICATES but is now defined {count} time(s) — \
             remove the stale entry so a future collision on this name is caught"
        );
    }
}

/// Every `.rs` file in the workspace's crates.
fn rust_sources() -> Vec<PathBuf> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("fubbik-api has a parent directory");
    let mut out = Vec::new();
    walk(crates, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // `target/` holds generated code that is not ours to police.
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Returns `(registered_schema_name, rust_ident)` for each `ToSchema` type.
///
/// `#[schema(as = Other)]` overrides the registered name, which is exactly the
/// escape hatch used to resolve a collision — so it must be honored here, or
/// the fix would still read as a collision.
fn schema_names_in(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let lines: Vec<&str> = src.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        if !(line.contains("derive(") && line.contains("ToSchema")) {
            continue;
        }
        // Scan forward past doc comments and other attributes to the item,
        // picking up an `as =` rename on the way.
        let mut alias: Option<String> = None;
        for next in lines.iter().skip(i + 1).take(30) {
            let t = next.trim();
            if let Some(rest) = t.strip_prefix("#[schema(")
                && let Some(pos) = rest.find("as = ")
            {
                let name: String = rest[pos + 5..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    alias = Some(name);
                }
            }
            for kw in ["pub struct ", "pub enum "] {
                if let Some(rest) = t.strip_prefix(kw) {
                    let ident: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !ident.is_empty() {
                        out.push((alias.clone().unwrap_or_else(|| ident.clone()), ident));
                    }
                }
            }
            if t.starts_with("pub struct ") || t.starts_with("pub enum ") {
                break;
            }
        }
    }
    out
}
