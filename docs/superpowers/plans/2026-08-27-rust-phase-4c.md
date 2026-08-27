# Rust Phase 4c — Context Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the request/response context pipeline to Rust — populate `fubbik-core` with the tokenizer, scorer, budgeter and formatter, then the eight context endpoints that depend on them.

**Architecture:** `fubbik-core` becomes the home for pure knowledge-scoring logic: no database, no HTTP, no Ollama, so every function is unit-testable with no fixtures. `health_score` moves there from `fubbik-api` so the scorer's dependency stops pointing the wrong way. Token counting uses `tiktoken-rs` with `o200k_base` for exact parity with Node's `encodingForModel("gpt-4o")`, because the tokenizer decides which chunks survive budgeting. The endpoints stay in `fubbik-api` and compose core's primitives.

**Tech Stack:** Rust 2024, axum 0.8, sqlx 0.8 (offline `.sqlx` cache), utoipa 5, tiktoken-rs, wiremock, tokio, Postgres 18 + pgvector.

**Spec:** `docs/superpowers/specs/2026-08-27-rust-phase-4c-design.md`

## Global Constraints

- `export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"` before any cargo command touching sqlx macros. **This is a Homebrew Postgres 18, not Docker** — the project's Docker setup would not start.
- That database has **pgvector and pg_trgm but no Apache AGE**. Two tests in `crates/fubbik-db/tests/staleness.rs` (`flag_impact_ripple_*`) fail there by design and are pre-existing. AGE-gated tests **skip silently and report as "passed"** — `cargo test` suppresses `eprintln!` without `--nocapture`. Never read a green suite as proof an AGE path ran.
- `cargo test --workspace` **without `--no-fail-fast` stops at the first failing target** and runs a fraction of the suites while still exiting 0. Always pass `--no-fail-fast`.
- `cargo sqlx prepare --workspace` alone **silently drops test-target-only entries**. Always `cargo sqlx prepare --workspace -- --tests`.
- `#[sqlx::test]` convention is **crate-dependent**: `crates/fubbik-api/tests` uses `migrations = "../fubbik-db/migrations"` (40 of 41 files); `crates/fubbik-db/tests` uses the **bare** form (38 of 39 files).
- Any task adding or changing a route must regenerate the committed spec: `cargo run -- openapi > openapi.json`, then `cargo test -p fubbik-api --test openapi`. Confirm the diff is confined to the new paths.
- Node parity is the default. Where the Rust diverges it must say so and why; silence means it must match.
- Explicit pathspecs on commit. Never `git add -A`. No `Co-Authored-By` or "Generated with Claude" trailers. **Never `git push`.**
- `cargo fmt --all` before every commit; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- **Implementers run focused tests only.** The controller runs the workspace suite between tasks. Do not background a long cargo run and wait on it.
- Do not start the Node server. Do not start any server.
- **Tests are written as real code, never as prose comments.** A comment describing a test is a plan failure, not an instruction.

---

## File Structure

**`crates/fubbik-core/src/`** — currently `error.rs` + a one-line `lib.rs`. Gains:
- `health.rs` — moved verbatim from `crates/fubbik-api/src/chunks/health_score.rs`
- `tokens.rs` — `estimate_tokens`
- `score.rs` — `ScoredChunk`, `score_chunk`, `budget_chunks`
- `format.rs` — `format_chunk_text`, `format_structured`, section mapping

**`crates/fubbik-api/src/context/`** (new) — `mod.rs`, `resolvers.rs`, `service.rs`, `dto.rs`, `routes.rs`, `snapshot.rs`
**`crates/fubbik-api/src/context_for_file/`** (new) — `mod.rs`, `service.rs`, `dto.rs`, `routes.rs`
**`crates/fubbik-api/src/context_export/`** (new) — `mod.rs`, `service.rs`, `claude_md.rs`, `routes.rs`
**`crates/fubbik-api/src/generate_instructions/`** (new) — `mod.rs`, `service.rs`, `routes.rs`

**Modified:** `crates/fubbik-api/src/{lib,openapi}.rs`, `chunks/{service,dto}.rs`, `search/service.rs`, `crates/fubbik-core/Cargo.toml`, `CLAUDE.md`.

---

## Task 0: Baseline

**Files:** none (measurement only)

**Interfaces:**
- Consumes: nothing
- Produces: the baseline every later task compares against

- [ ] **Step 1: Confirm the database is reachable and has the right extensions**

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
psql "$DATABASE_URL" -tAc "SELECT extname FROM pg_extension ORDER BY 1;"
```

Expected: includes `vector` and `pg_trgm`. `age` will be **absent** — that is expected on this machine.

- [ ] **Step 2: Record the baseline**

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
cargo test --workspace --no-fail-fast 2>&1 | grep -E "^test result:" | awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'
```

Expected: **1166 passed, 2 failed, 13 ignored** (Phase 4b's exit state). The 2 failures are `flag_impact_ripple_does_not_flag_a_cross_user_ripple_target` and `flag_impact_ripple_rerun_does_not_accumulate_duplicate_flags`. If the numbers differ, report before continuing.

- [ ] **Step 3: Capture openapi.json's hash — Task 1 must not change it**

```bash
shasum -a 256 openapi.json
```

Write the hash down. Task 1 moves a registered utoipa schema and must leave this identical.

- [ ] **Step 4: Confirm a clean tree**

```bash
git status --short
```

Expected: empty.

---

## Task 1: Move `health_score` into `fubbik-core`

The riskiest task in the slice: it touches shipped, tested code and a registered API schema. It lands first so nothing new is entangled with it.

**Files:**
- Create: `crates/fubbik-core/src/health.rs` (moved content)
- Delete: `crates/fubbik-api/src/chunks/health_score.rs`
- Modify: `crates/fubbik-core/src/lib.rs`, `crates/fubbik-core/Cargo.toml`
- Modify: `crates/fubbik-api/src/openapi.rs:320-321`, `crates/fubbik-api/src/search/service.rs:85,629`, `crates/fubbik-api/src/chunks/service.rs:524`, `crates/fubbik-api/src/chunks/dto.rs:285`, `crates/fubbik-api/src/chunks/mod.rs`

**Interfaces:**
- Consumes: nothing
- Produces: `fubbik_core::health::{ChunkHealthInput, HealthScore, HealthScoreBreakdown, compute_health_score}` — the same names and signatures as before the move.

- [ ] **Step 1: Add utoipa to fubbik-core**

`HealthScore` and `HealthScoreBreakdown` derive `utoipa::ToSchema`, so the crate needs it. In `crates/fubbik-core/Cargo.toml`, under `[dependencies]`:

```toml
utoipa = { version = "5", features = ["axum_extras", "chrono"] }
```

This matches `crates/fubbik-db/Cargo.toml`'s existing line exactly — the workspace already carries utoipa in a non-API crate, so this is consistent rather than novel.

- [ ] **Step 2: Move the file verbatim**

```bash
git mv crates/fubbik-api/src/chunks/health_score.rs crates/fubbik-core/src/health.rs
```

Using `git mv` keeps the rename visible in history. **Do not change the file's contents in this step** — not the logic, not the doc comments, not the tests. A move and an edit in one commit make it impossible to tell which broke something.

- [ ] **Step 3: Register the module**

`crates/fubbik-core/src/lib.rs`:

```rust
pub mod error;
pub mod health;
```

And remove `pub mod health_score;` (or `mod health_score;`) from `crates/fubbik-api/src/chunks/mod.rs`.

- [ ] **Step 4: Update all five call sites**

- `crates/fubbik-api/src/openapi.rs:320-321`: `crate::chunks::health_score::HealthScore` → `fubbik_core::health::HealthScore`, same for `HealthScoreBreakdown`.
- `crates/fubbik-api/src/search/service.rs:85`: `use crate::chunks::health_score::{self, ChunkHealthInput};` → `use fubbik_core::health::{self, ChunkHealthInput};`
- `crates/fubbik-api/src/chunks/service.rs:524`: `super::health_score::compute_health_score(&super::health_score::ChunkHealthInput { … })` → `fubbik_core::health::compute_health_score(&fubbik_core::health::ChunkHealthInput { … })`
- `crates/fubbik-api/src/chunks/dto.rs:285`: `crate::chunks::health_score::HealthScore` → `fubbik_core::health::HealthScore`

- [ ] **Step 5: Build and run the affected suites**

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
cargo test -p fubbik-core
cargo test -p fubbik-api --test chunks
cargo test -p fubbik-api --test chunk_detail
cargo test -p fubbik-api --test search
```

Expected: all pass. The health-score unit tests moved with the file and must still pass in their new crate.

- [ ] **Step 6: THE GUARD — `openapi.json` must be byte-identical**

```bash
cargo run -- openapi > /tmp/openapi-after-move.json
diff openapi.json /tmp/openapi-after-move.json && echo "IDENTICAL — schema names survived the move"
```

Expected: no diff. utoipa derives schema names from the type name, not the module path, so `HealthScore` should stay `HealthScore`.

**If there IS a diff, STOP and report it.** A changed schema name means the move silently altered the public API and the web client's generated types would change. Do not regenerate `openapi.json` to make the difference go away — that would bake the break in.

- [ ] **Step 7: Lint, format, commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-core crates/fubbik-api
git commit -m "refactor(core): move health scoring into fubbik-core

score_chunk needs compute_health_score, so leaving health scoring in
fubbik-api would point a dependency the wrong way once the scorer lands
in core. It is pure logic that never touched the database.

Five call sites updated across four files. HealthScore and
HealthScoreBreakdown are registered utoipa schemas and HealthScore is a
field on ChunkDetail, so openapi.json was diffed before and after: it is
byte-identical, confirming utoipa derives schema names from the type name
rather than the module path."
```

---

## Task 2: The tokenizer

**Files:**
- Create: `crates/fubbik-core/src/tokens.rs`
- Modify: `crates/fubbik-core/src/lib.rs`, `crates/fubbik-core/Cargo.toml`

**Interfaces:**
- Consumes: nothing
- Produces: `fubbik_core::tokens::estimate_tokens(text: &str) -> usize`

- [ ] **Step 1: Add the dependency**

In `crates/fubbik-core/Cargo.toml` under `[dependencies]`:

```toml
tiktoken-rs = "0.6"
```

Check the latest 0.x on docs.rs if 0.6 does not resolve, and record which version you used in your report along with the build-size delta (`ls -la target/debug/` before and after is enough for an order of magnitude).

- [ ] **Step 2: Write the failing tests**

`crates/fubbik-core/src/tokens.rs`, at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned against known-good counts so a future tiktoken-rs upgrade that
    /// changes the encoding is caught by a red test rather than by exports
    /// quietly changing which chunks fit in a budget.
    #[test]
    fn counts_match_o200k_base_for_known_strings() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hello"), 1);
        assert_eq!(estimate_tokens("hello world"), 2);
        // The budgeter's seed string — its count is load-bearing.
        assert_eq!(estimate_tokens("# Project Context\n\n"), 5);
    }

    /// A token is not a character. If this ever equals the character count,
    /// the real encoder has been replaced by a length-based approximation.
    #[test]
    fn is_not_a_character_count() {
        let text = "The quick brown fox jumps over the lazy dog";
        let tokens = estimate_tokens(text);
        assert!(tokens > 0);
        assert!(
            tokens < text.chars().count(),
            "a real tokenizer produces fewer tokens than characters for English prose; \
             got {tokens} tokens for {} chars",
            text.chars().count()
        );
    }

    #[test]
    fn handles_non_ascii_without_panicking() {
        assert!(estimate_tokens("héllo wörld — ünïcode") > 0);
        assert!(estimate_tokens("日本語のテキスト") > 0);
    }
}
```

**The exact counts in the first test are a starting point, not gospel.** Run the test, and if a count differs, verify the actual value is correct for `o200k_base` before changing the assertion — then record in your report what you changed and why. Do not simply paste in whatever the implementation returned; that would make the test a mirror of the code rather than a check on it.

- [ ] **Step 3: Run to verify failure**

```bash
cargo test -p fubbik-core tokens
```

Expected: FAIL — `estimate_tokens` does not exist.

- [ ] **Step 4: Implement**

Above the test module in `crates/fubbik-core/src/tokens.rs`:

```rust
//! Token counting for context budgeting.
//!
//! Ports `packages/api/src/context/utils.ts:19-37`, which calls
//! `encodingForModel("gpt-4o")` from `js-tiktoken`. `gpt-4o` maps to the
//! `o200k_base` encoding.
//!
//! This is not a cosmetic parity choice. `score::budget_chunks` greedily
//! fills a token budget, so the tokenizer decides *which chunks appear in
//! an export*. A `len / 4` approximation would under-count code-heavy
//! content, over-fill budgets, and select a different set of chunks than
//! Node for the same request — a divergence no test could catch without
//! comparing the two backends directly.
//!
//! Node's `Math.ceil(text.length / 4)` fallback is deliberately NOT ported.
//! It exists because `js-tiktoken` loads its BPE data lazily and can fail;
//! `tiktoken-rs` embeds that data at compile time and cannot fail the same
//! way. Porting a branch that can never execute would add untestable code
//! whose only effect, if it somehow fired, would be to silently change what
//! an export contains.
use std::sync::OnceLock;

use tiktoken_rs::CoreBPE;

fn encoder() -> &'static CoreBPE {
    static ENCODER: OnceLock<CoreBPE> = OnceLock::new();
    ENCODER.get_or_init(|| {
        tiktoken_rs::o200k_base().expect("o200k_base is embedded at compile time")
    })
}

pub fn estimate_tokens(text: &str) -> usize {
    encoder().encode_ordinary(text).len()
}
```

`encode_ordinary` skips special-token handling, which is what `js-tiktoken`'s plain `encode` does for arbitrary text. If the crate's API differs in the version you pulled, match the *behaviour* — count the tokens of the literal text with no special-token interpretation — and say what you used.

- [ ] **Step 5: Run to verify pass**

```bash
cargo test -p fubbik-core tokens
```

Expected: PASS, 3 tests.

- [ ] **Step 6: Register and commit**

Add `pub mod tokens;` to `crates/fubbik-core/src/lib.rs`.

```bash
cargo fmt --all
cargo clippy -p fubbik-core --all-targets -- -D warnings
git add crates/fubbik-core
git commit -m "feat(core): add o200k_base token counting

Exact parity with Node's encodingForModel(\"gpt-4o\"). The tokenizer
decides which chunks survive budgeting, so an approximation would select
different content than Node for the same request with nothing to catch it.

Node's char/4 fallback is not ported: it exists because js-tiktoken loads
BPE data lazily and can fail, while tiktoken-rs embeds it at compile time."
```

---

## Task 3: Scorer, budgeter and chunk formatting

**Files:**
- Create: `crates/fubbik-core/src/score.rs`, `crates/fubbik-core/src/format.rs`
- Modify: `crates/fubbik-core/src/lib.rs`

**Interfaces:**
- Consumes: `fubbik_core::tokens::estimate_tokens` (Task 2), `fubbik_core::health::{compute_health_score, ChunkHealthInput}` (Task 1)
- Produces:
  - `pub struct ScoredChunk { pub id: String, pub title: String, pub content: String, pub chunk_type: String, pub rationale: Option<String>, pub tags: Vec<String>, pub score: f64 }`
  - `pub struct ScoreInput<'a> { pub chunk_type: &'a str, pub rationale: Option<&'a str>, pub review_status: &'a str, pub connection_count: i64, pub health: &'a HealthScore }`
  - `score_chunk(input: &ScoreInput) -> f64`
  - `budget_chunks(chunks: Vec<ScoredChunk>, max_tokens: usize) -> Vec<ScoredChunk>`
  - `format_chunk_text(chunk: &ScoredChunk) -> String`

- [ ] **Step 1: Write the failing tests**

`crates/fubbik-core/src/score.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{ChunkHealthInput, compute_health_score};

    fn health_for(content: &str, rationale: Option<&str>, connections: i64) -> crate::health::HealthScore {
        compute_health_score(&ChunkHealthInput {
            content,
            summary: None,
            rationale,
            alternatives: None,
            consequences: None,
            connection_count: connections,
            centrality_degree: 0,
            has_embedding: false,
            requirement_count: 0,
            all_requirements_passing: false,
            referenced_in_session: false,
        })
    }

    fn scored(id: &str, score: f64, content: &str) -> ScoredChunk {
        ScoredChunk {
            id: id.into(),
            title: "T".into(),
            content: content.into(),
            chunk_type: "note".into(),
            rationale: None,
            tags: vec![],
            score,
        }
    }

    #[test]
    fn type_points_are_three_one_two() {
        let h = health_for("x", None, 0);
        let base = |t: &str| {
            score_chunk(&ScoreInput {
                chunk_type: t,
                rationale: None,
                review_status: "draft",
                connection_count: 0,
                health: &h,
            })
        };
        assert_eq!(base("document") - base("reference"), 1.0);
        assert_eq!(base("reference") - base("note"), 1.0);
    }

    #[test]
    fn rationale_adds_exactly_two() {
        let h = health_for("x", None, 0);
        let without = score_chunk(&ScoreInput {
            chunk_type: "note", rationale: None, review_status: "draft",
            connection_count: 0, health: &h,
        });
        let with = score_chunk(&ScoreInput {
            chunk_type: "note", rationale: Some("because"), review_status: "draft",
            connection_count: 0, health: &h,
        });
        assert_eq!(with - without, 2.0);
    }

    #[test]
    fn connection_points_cap_at_ten() {
        let h = health_for("x", None, 0);
        let at = |n: i64| score_chunk(&ScoreInput {
            chunk_type: "note", rationale: None, review_status: "draft",
            connection_count: n, health: &h,
        });
        assert_eq!(at(5) - at(0), 10.0);
        assert_eq!(at(50), at(5), "connection points must cap at 10, not keep growing");
    }

    #[test]
    fn review_points_are_two_one_zero() {
        let h = health_for("x", None, 0);
        let at = |s: &str| score_chunk(&ScoreInput {
            chunk_type: "note", rationale: None, review_status: s,
            connection_count: 0, health: &h,
        });
        assert_eq!(at("approved") - at("draft"), 2.0);
        assert_eq!(at("reviewed") - at("draft"), 1.0);
    }

    /// Freshness lives inside compute_health_score and contributes through
    /// `health.total / 10`. A separate freshness term would double-count it.
    /// This pins the health contribution to exactly that ratio: if someone
    /// adds a freshness bonus on top, the difference stops matching.
    #[test]
    fn health_contributes_exactly_total_over_ten_and_nothing_else() {
        let lean = health_for("x", None, 0);
        let rich = health_for(&"y".repeat(2000), Some("because"), 0);
        assert_ne!(lean.total, rich.total, "fixture must produce differing health totals");

        let at = |h: &crate::health::HealthScore| score_chunk(&ScoreInput {
            chunk_type: "note", rationale: None, review_status: "draft",
            connection_count: 0, health: h,
        });

        let expected = (rich.total as f64 / 10.0) - (lean.total as f64 / 10.0);
        assert!(
            (at(&rich) - at(&lean) - expected).abs() < f64::EPSILON,
            "health must contribute exactly total/10 — a separate freshness term would break this"
        );
    }

    /// The budgeter SKIPS an oversized chunk and keeps going; it does not
    /// stop at the first one that will not fit. A test asserting only "the
    /// result fits the budget" passes with a `break` in place of `continue`.
    #[test]
    fn budget_skips_an_oversized_chunk_rather_than_truncating() {
        let huge = scored("huge", 100.0, &"word ".repeat(5000));
        let small_a = scored("a", 50.0, "small");
        let small_b = scored("b", 40.0, "small");

        let kept = budget_chunks(vec![huge, small_a, small_b], 200);
        let ids: Vec<&str> = kept.iter().map(|c| c.id.as_str()).collect();

        assert!(!ids.contains(&"huge"), "the oversized chunk must be skipped");
        assert_eq!(ids, vec!["a", "b"], "lower-scored chunks that fit must still be selected");
    }

    #[test]
    fn budget_returns_highest_scored_first() {
        let kept = budget_chunks(
            vec![scored("low", 1.0, "x"), scored("high", 99.0, "x"), scored("mid", 50.0, "x")],
            10_000,
        );
        assert_eq!(
            kept.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["high", "mid", "low"]
        );
    }

    /// The running total is seeded with the header's own token count, so a
    /// budget smaller than the header admits nothing.
    #[test]
    fn budget_accounts_for_the_header_seed() {
        let kept = budget_chunks(vec![scored("a", 1.0, "x")], 1);
        assert!(kept.is_empty(), "a budget below the header's own cost admits nothing");
    }
}
```

`crates/fubbik-core/src/format.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::ScoredChunk;

    fn chunk(chunk_type: &str, rationale: Option<&str>, content: &str) -> ScoredChunk {
        ScoredChunk {
            id: "i".into(),
            title: "Title".into(),
            content: content.into(),
            chunk_type: chunk_type.into(),
            rationale: rationale.map(str::to_string),
            tags: vec![],
            score: 0.0,
        }
    }

    #[test]
    fn document_becomes_architecture() {
        assert!(format_chunk_text(&chunk("document", None, "body"))
            .starts_with("## Architecture: Title"));
    }

    #[test]
    fn note_becomes_note_and_convention_becomes_convention() {
        assert!(format_chunk_text(&chunk("note", None, "body")).starts_with("## Note: Title"));
        assert!(format_chunk_text(&chunk("convention", None, "body"))
            .starts_with("## Convention: Title"));
    }

    /// Unknown types are title-cased rather than passed through, matching
    /// Node's `charAt(0).toUpperCase() + slice(1)`.
    #[test]
    fn unknown_type_is_title_cased() {
        assert!(format_chunk_text(&chunk("runbook", None, "body"))
            .starts_with("## Runbook: Title"));
    }

    #[test]
    fn rationale_is_appended_with_its_label() {
        let out = format_chunk_text(&chunk("note", Some("because"), "body"));
        assert_eq!(out, "## Note: Title\nbody\n**Rationale:** because");
    }

    /// Node pushes content only when it is truthy, so an empty string is
    /// omitted rather than producing a blank line.
    #[test]
    fn empty_content_is_omitted_not_blank() {
        assert_eq!(format_chunk_text(&chunk("note", None, "")), "## Note: Title");
    }
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-core
```

Expected: FAIL — nothing in `score` or `format` exists yet.

- [ ] **Step 3: Implement the scorer and budgeter**

`crates/fubbik-core/src/score.rs`, above the tests:

```rust
//! Chunk scoring and token budgeting.
//!
//! Ports `packages/api/src/context/utils.ts:39-76`.
//!
//! `budgetChunksWithCoverage` (`utils.ts:81`) is deliberately NOT ported:
//! it has zero callers, and its only source of `communityId` is
//! `packages/api/src/graph/community-analysis.ts`, which Phase 4a
//! established has no clients either. Porting it would mean writing
//! untestable code for a path that cannot execute.
use crate::format::format_chunk_text;
use crate::health::HealthScore;
use crate::tokens::estimate_tokens;

/// Node seeds the running total with this header's own token count
/// (`utils.ts:66`).
const HEADER_SEED: &str = "# Project Context\n\n";

#[derive(Debug, Clone)]
pub struct ScoredChunk {
    pub id: String,
    pub title: String,
    pub content: String,
    pub chunk_type: String,
    pub rationale: Option<String>,
    pub tags: Vec<String>,
    pub score: f64,
}

pub struct ScoreInput<'a> {
    pub chunk_type: &'a str,
    pub rationale: Option<&'a str>,
    pub review_status: &'a str,
    pub connection_count: i64,
    pub health: &'a HealthScore,
}

/// Sum of five terms. Health contributes `total / 10` and there is
/// deliberately **no separate freshness term** — freshness is already
/// inside `compute_health_score`, and adding one here would double-count
/// it. Node carries the same warning as a comment at `utils.ts:59`.
pub fn score_chunk(input: &ScoreInput) -> f64 {
    let health_points = input.health.total as f64 / 10.0;
    let type_points = match input.chunk_type {
        "document" => 3.0,
        "note" => 1.0,
        _ => 2.0,
    };
    let rationale_points = if input.rationale.is_some() { 2.0 } else { 0.0 };
    let connection_points = (input.connection_count * 2).min(10) as f64;
    let review_points = match input.review_status {
        "approved" => 2.0,
        "reviewed" => 1.0,
        _ => 0.0,
    };
    health_points + type_points + rationale_points + connection_points + review_points
}

/// Greedily fills a token budget, highest score first.
///
/// A chunk that would exceed the budget is **skipped**, not a stopping
/// point: Node uses `continue`, so one oversized chunk does not truncate
/// the export while smaller lower-scored chunks still fit.
pub fn budget_chunks(chunks: Vec<ScoredChunk>, max_tokens: usize) -> Vec<ScoredChunk> {
    let mut sorted = chunks;
    sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    let mut selected = Vec::new();
    let mut used = estimate_tokens(HEADER_SEED);

    for chunk in sorted {
        let tokens = estimate_tokens(&format_chunk_text(&chunk));
        if used + tokens > max_tokens {
            continue;
        }
        used += tokens;
        selected.push(chunk);
    }
    selected
}
```

- [ ] **Step 4: Implement the formatter**

`crates/fubbik-core/src/format.rs`, above the tests:

```rust
//! Chunk-to-markdown formatting.
//!
//! Ports `packages/api/src/context/utils.ts:108-126` (`formatChunkText`).
use crate::score::ScoredChunk;

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn format_chunk_text(chunk: &ScoredChunk) -> String {
    let type_label = match chunk.chunk_type.as_str() {
        "document" => "Architecture".to_string(),
        "convention" => "Convention".to_string(),
        "note" => "Note".to_string(),
        other => title_case(other),
    };

    let mut parts = vec![format!("## {type_label}: {}", chunk.title)];
    // Node pushes content only when truthy, so an empty string is omitted
    // rather than contributing a blank line.
    if !chunk.content.is_empty() {
        parts.push(chunk.content.clone());
    }
    if let Some(rationale) = &chunk.rationale {
        parts.push(format!("**Rationale:** {rationale}"));
    }
    parts.join("\n")
}
```

- [ ] **Step 5: Run to verify pass**

```bash
cargo test -p fubbik-core
```

Expected: PASS — 8 score tests, 5 format tests, plus Tasks 1 and 2's.

- [ ] **Step 6: Mutation-test the three that matter**

Each needs real runtime output naming the test — a compile error is not evidence.

1. Change `continue` to `break` in `budget_chunks`. `budget_skips_an_oversized_chunk_rather_than_truncating` must FAIL.
2. Add a freshness term to `score_chunk` (e.g. `+ 1.0` unconditionally is not enough — add something derived from health, such as `+ input.health.breakdown.freshness as f64`). `health_contributes_exactly_total_over_ten_and_nothing_else` must FAIL.
3. Change `.min(10)` to `.min(100)` in the connection term. `connection_points_cap_at_ten` must FAIL.

Revert each and report the actual output. If any mutation leaves its test passing, say so plainly rather than claiming a failure you did not observe — that test is then not pinning what it claims.

- [ ] **Step 7: Register and commit**

Add `pub mod format;` and `pub mod score;` to `crates/fubbik-core/src/lib.rs`.

```bash
cargo fmt --all
cargo clippy -p fubbik-core --all-targets -- -D warnings
git add crates/fubbik-core
git commit -m "feat(core): add the chunk scorer, budgeter and formatter

Ports context/utils.ts's scoreChunk, budgetChunks and formatChunkText.

Health contributes total/10 with no separate freshness term — freshness
is already inside compute_health_score and a second term would double
count it. A test pins the contribution to exactly that ratio.

The budgeter skips an oversized chunk rather than stopping at it, so one
large chunk cannot truncate an export while smaller ones still fit.

budgetChunksWithCoverage is not ported: zero callers, and its only
communityId producer was established caller-less in Phase 4a."
```

---

## Task 4: Structured formatting into sections

**Files:**
- Modify: `crates/fubbik-core/src/format.rs`

**Interfaces:**
- Consumes: `ScoredChunk` (Task 3)
- Produces:
  - `pub struct ChunkWithMetadata { pub chunk: ScoredChunk, pub health_score: i64, pub is_stale: bool, pub has_pending_proposal: bool }`
  - `pub struct ContextSection { pub title: String, pub chunks: Vec<ChunkWithMetadata> }`
  - `pub struct StructuredContext { pub sections: Vec<ContextSection>, pub total_chunks: usize }`
  - `format_structured(chunks: Vec<ChunkWithMetadata>) -> StructuredContext`

- [ ] **Step 1: Write the failing tests**

Append to `crates/fubbik-core/src/format.rs`'s test module:

```rust
    fn with_meta(chunk_type: &str, tags: &[&str]) -> ChunkWithMetadata {
        ChunkWithMetadata {
            chunk: ScoredChunk {
                id: "i".into(),
                title: "T".into(),
                content: "c".into(),
                chunk_type: chunk_type.into(),
                rationale: None,
                tags: tags.iter().map(|t| t.to_string()).collect(),
                score: 0.0,
            },
            health_score: 50,
            is_stale: false,
            has_pending_proposal: false,
        }
    }

    #[test]
    fn types_map_to_their_section_titles() {
        let out = format_structured(vec![
            with_meta("note", &[]),
            with_meta("document", &[]),
            with_meta("reference", &[]),
            with_meta("schema", &[]),
            with_meta("checklist", &[]),
        ]);
        let titles: Vec<&str> = out.sections.iter().map(|s| s.title.as_str()).collect();
        assert!(titles.contains(&"Notes"));
        assert!(titles.contains(&"Architecture"));
        assert!(titles.contains(&"API Reference"));
        assert!(titles.contains(&"Schemas"));
        assert!(titles.contains(&"Checklists"));
    }

    /// A note tagged `convention` is pulled out of Notes into its own
    /// section — the one case where the tag, not the type, decides.
    #[test]
    fn a_note_tagged_convention_becomes_its_own_section() {
        let out = format_structured(vec![with_meta("note", &["convention"])]);
        assert_eq!(out.sections.len(), 1);
        assert_eq!(out.sections[0].title, "Conventions");
    }

    #[test]
    fn a_note_without_the_tag_stays_in_notes() {
        let out = format_structured(vec![with_meta("note", &["other"])]);
        assert_eq!(out.sections[0].title, "Notes");
    }

    #[test]
    fn chunks_of_one_type_group_into_a_single_section() {
        let out = format_structured(vec![with_meta("note", &[]), with_meta("note", &[])]);
        assert_eq!(out.sections.len(), 1);
        assert_eq!(out.sections[0].chunks.len(), 2);
        assert_eq!(out.total_chunks, 2);
    }

    #[test]
    fn unknown_type_gets_a_title_cased_section() {
        let out = format_structured(vec![with_meta("runbook", &[])]);
        assert_eq!(out.sections[0].title, "Runbook");
    }
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-core format
```

Expected: FAIL — `format_structured` and `ChunkWithMetadata` do not exist.

- [ ] **Step 3: Implement**

Add to `crates/fubbik-core/src/format.rs`:

```rust
/// A scored chunk plus the enrichment the formatter annotates with.
/// Ports `packages/api/src/context/formatter.ts:3-7`.
#[derive(Debug, Clone)]
pub struct ChunkWithMetadata {
    pub chunk: ScoredChunk,
    pub health_score: i64,
    pub is_stale: bool,
    pub has_pending_proposal: bool,
}

#[derive(Debug, Clone)]
pub struct ContextSection {
    pub title: String,
    pub chunks: Vec<ChunkWithMetadata>,
}

#[derive(Debug, Clone)]
pub struct StructuredContext {
    pub sections: Vec<ContextSection>,
    pub total_chunks: usize,
}

/// Ports `formatter.ts:20-35`. The `convention` tag is the single case
/// where a tag rather than the type decides the section.
fn section_title(c: &ChunkWithMetadata) -> String {
    if c.chunk.chunk_type == "note" && c.chunk.tags.iter().any(|t| t == "convention") {
        return "Conventions".to_string();
    }
    match c.chunk.chunk_type.as_str() {
        "note" => "Notes".to_string(),
        "document" => "Architecture".to_string(),
        "reference" => "API Reference".to_string(),
        "schema" => "Schemas".to_string(),
        "checklist" => "Checklists".to_string(),
        other => title_case(other),
    }
}

/// Groups chunks into sections, preserving first-seen section order.
///
/// Node builds a `Map` and iterates its entries, and JS `Map` iteration is
/// insertion-ordered — so section order follows the order sections were
/// first encountered, not alphabetical or type order. An `IndexMap`-style
/// `Vec` scan reproduces that without a new dependency.
pub fn format_structured(chunks: Vec<ChunkWithMetadata>) -> StructuredContext {
    let total_chunks = chunks.len();
    let mut sections: Vec<ContextSection> = Vec::new();

    for c in chunks {
        let title = section_title(&c);
        match sections.iter_mut().find(|s| s.title == title) {
            Some(section) => section.chunks.push(c),
            None => sections.push(ContextSection { title, chunks: vec![c] }),
        }
    }

    StructuredContext { sections, total_chunks }
}
```

- [ ] **Step 4: Run to verify pass**

```bash
cargo test -p fubbik-core format
```

Expected: PASS, 10 format tests total.

- [ ] **Step 5: Mutation-test the section mapping**

Remove the `convention`-tag branch from `section_title`. `a_note_tagged_convention_becomes_its_own_section` must FAIL. Revert and report the output.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
cargo clippy -p fubbik-core --all-targets -- -D warnings
git add crates/fubbik-core/src/format.rs
git commit -m "feat(core): group chunks into context sections

Ports formatter.ts's formatStructured. Section order follows first
encounter rather than any sort, matching JS Map iteration order, which is
insertion-ordered — a Vec scan reproduces it without a new dependency.

A note tagged 'convention' is the single case where a tag rather than the
chunk type picks the section."
```

---

## Task 5: Context resolvers and enrichment

**Files:**
- Create: `crates/fubbik-api/src/context/{mod,resolvers,service,dto}.rs`
- Modify: `crates/fubbik-api/src/lib.rs`
- Test: `crates/fubbik-api/tests/context.rs` (new)

**Interfaces:**
- Consumes: `fubbik_core::{score, format, tokens}` (Tasks 2-4), `fubbik_db::repo::semantic::semantic_search` and `fubbik_ai::OllamaClient::embed_query` (Phase 4b)
- Produces:
  - `resolvers::resolve_for_plan(pool, user_id, plan_id) -> AppResult<Vec<String>>`
  - `resolvers::resolve_for_concept(pool, ai, user_id, query, space_id) -> AppResult<Vec<String>>`
  - `resolvers::resolve_for_files(pool, user_id, paths, space_id) -> AppResult<Vec<String>>`
  - `service::enrich_chunks(pool, user_id, ids) -> AppResult<Vec<ChunkWithMetadata>>`

**Read `packages/api/src/context/resolvers.ts` (235 LOC) before starting.** Each resolver produces candidate chunk ids; `enrichChunks` then fetches full rows, connections, tags, stale flags, proposals and active feature overlays, and computes health scores.

- [ ] **Step 1: Write the failing tests**

`crates/fubbik-api/tests/context.rs` — copy the `state` / `signup` / `send` / `json_body` helpers from `crates/fubbik-api/tests/enrich.rs`, then:

```rust
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn resolve_for_plan_returns_chunks_linked_through_tasks(pool: sqlx::PgPool) {
    // Create a plan with a task, link two chunks to that task, and assert
    // both ids come back while an unlinked chunk does not.
    // See packages/api/src/context/resolvers.ts's resolveForPlan for the
    // exact link path (plan_task_chunk).
    todo!("write this out fully — see the note below")
}
```

**This task's tests must be written as real code before implementation, not left as `todo!()`.** The stub above shows only the shape and the helper set to copy. Read `resolvers.ts`, determine the exact link path, and write the assertions. The tests required:

- `resolve_for_plan_returns_chunks_linked_through_tasks` — linked chunks returned, unlinked chunk absent
- `resolve_for_plan_is_scoped_to_the_owner` — user B resolving user A's plan gets nothing (or a `NotFound`, whichever `resolvers.ts` does — match it)
- `resolve_for_concept_combines_semantic_and_text_matches` — with a wiremock `/api/embeddings`, a semantically-near chunk and a title-matching chunk both appear
- `resolve_for_files_matches_file_refs_and_applies_to_globs` — a chunk linked by `chunk_file_ref` and one matching via an `applies_to` glob both appear
- `enrich_chunks_computes_health_and_flags_stale` — a stale-flagged chunk comes back with `is_stale: true` and a health score in `0..=100`

- [ ] **Step 2: Run to verify failure**

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
cargo test -p fubbik-api --test context
```

Expected: FAIL — the module does not exist.

- [ ] **Step 3: Implement**

Port `resolvers.ts` function by function. Keep each resolver returning **candidate ids only** — enrichment is a separate step, which is what lets all three input sources share one pipeline.

`enrich_chunks` fetches rows via `fubbik_db::repo::chunk`, connection counts via `repo::connection`, tags via `repo::tag`, stale flags via `repo::staleness`, and computes `fubbik_core::health::compute_health_score` per chunk, then `fubbik_core::score::score_chunk`. Feature overlays apply as in `packages/api/src/features/service.ts`'s `resolveFeatureOverlays` — `Object.assign(base, ...deltas ascending by priority)`.

- [ ] **Step 4: Run to verify pass**

```bash
cargo test -p fubbik-api --test context
```

Expected: PASS, 5 tests.

- [ ] **Step 5: Mutation-test the scoping**

Remove the user filter from `resolve_for_plan`. `resolve_for_plan_is_scoped_to_the_owner` must FAIL. Revert and report the output.

- [ ] **Step 6: Refresh the query cache and commit**

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api crates/fubbik-db .sqlx
git commit -m "feat(api): add context resolvers and enrichment

Ports context/resolvers.ts. Each resolver produces candidate ids only;
enrichment is a separate step, which is what lets plan, concept and file
inputs share one pipeline."
```

---

## Task 6: `/context/for-plan`, `/context/about`, `/context/for-files`

**Files:**
- Create: `crates/fubbik-api/src/context/routes.rs`
- Modify: `crates/fubbik-api/src/context/{mod,dto}.rs`, `crates/fubbik-api/src/{lib,openapi}.rs`
- Test: `crates/fubbik-api/tests/context.rs`

**Interfaces:**
- Consumes: Task 5's resolvers and `enrich_chunks`; `fubbik_core::score::budget_chunks`
- Produces: three registered routes

**Read `packages/api/src/context/routes.ts` (142 LOC).** All three share a shape: resolve → enrich → score → budget → format, differing only in the resolver and the query parameters.

- [ ] **Step 1: Write the failing tests**

Append to `crates/fubbik-api/tests/context.rs`. Write each out fully:

- `for_plan_returns_the_plans_chunks_within_budget` — assert the response contains a linked chunk's title and that a `maxTokens` of 50 excludes a large chunk a `maxTokens` of 50000 includes. **Assert the difference**, not merely that both return 200.
- `for_plan_404s_for_another_users_plan`
- `about_finds_a_chunk_by_concept` — wiremock `/api/embeddings`, assert the semantically-near chunk appears and a distant one does not
- `for_files_accepts_a_csv_of_paths` — two paths, chunks for both returned
- `format_defaults_to_structured_md_and_json_is_selectable` — assert the two formats produce genuinely different bodies, not just both-200

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-api --test context
```

Expected: the five new tests FAIL with 404 (routes not registered).

- [ ] **Step 3: Implement the routes**

Follow `crates/fubbik-api/src/chunks/routes.rs`'s shape for handler signatures, `CurrentUser` extraction and utoipa annotations. Query DTOs mirror Node's — values arrive as strings, so `maxTokens` is parsed rather than typed, keeping the wire contract identical.

- [ ] **Step 4: Run to verify pass, then regenerate the API spec**

```bash
cargo test -p fubbik-api --test context
cargo run -- openapi > openapi.json
cargo test -p fubbik-api --test openapi
```

Confirm the `openapi.json` diff is confined to the three new paths and their schemas. If it shows unrelated churn, STOP and report.

- [ ] **Step 5: Mutation-test the budget**

Change `budget_chunks(chunks, max_tokens)` to ignore `max_tokens` (pass `usize::MAX`). `for_plan_returns_the_plans_chunks_within_budget` must FAIL. Revert and report the output.

- [ ] **Step 6: Commit**

```bash
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api openapi.json .sqlx
git commit -m "feat(api): port /context/for-plan, /context/about and /context/for-files

All three share resolve -> enrich -> score -> budget -> format, differing
only in resolver and query parameters. maxTokens stays string-typed on the
wire, matching Node's schema."
```

---

## Task 7: `/context/for-file`

The largest single port in the slice — five ranking strategies with additive bonuses.

**Files:**
- Create: `crates/fubbik-api/src/context_for_file/{mod,service,dto,routes}.rs`
- Modify: `crates/fubbik-api/src/{lib,openapi}.rs`
- Test: `crates/fubbik-api/tests/context_for_file.rs` (new)

**Interfaces:**
- Consumes: Phase 4b's `embed_query` + `semantic_search`; `fubbik_db::repo::chunk_meta` for file refs and applies-to globs
- Produces: `GET /api/context/for-file`

**Read `packages/api/src/context-for-file/service.ts` (332 LOC) in full before starting.** Also read its two helpers, `glob-match.ts` (20 LOC) and `detect-deps.ts` (22 LOC), each of which has its own Node test file worth mirroring.

The five strategies and their bonuses (`service.ts`): file-ref **+20**, applies-to **+10**, dependency **+3**, semantic **+5** (requires Ollama, capped at 10 results), connected **+2**. Each result carries a `matchReason` naming which strategy found it.

- [ ] **Step 1: Write the failing tests**

Write each out fully in `crates/fubbik-api/tests/context_for_file.rs`:

- `file_ref_match_outranks_applies_to_match` — a chunk found by file-ref must sort above one found by applies-to. **This is the test that proves the bonuses are additive and correctly ordered**; asserting only that both appear would pass with every bonus set to the same value.
- `each_result_carries_its_match_reason` — assert the exact `matchReason` strings
- `semantic_strategy_is_skipped_when_ollama_is_unreachable` — point the client at `http://127.0.0.1:1`, assert 200 with the other strategies' results still present. Confirm against `service.ts` whether Node degrades or errors here and match it.
- `glob_matching_handles_star_and_double_star` — unit tests for the glob helper, mirroring `glob-match.test.ts`
- `dependency_detection_matches_a_space_name` — mirroring `detect-deps.test.ts`

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-api --test context_for_file
```

Expected: FAIL.

- [ ] **Step 3: Implement**

Port the five strategies, accumulating a score per chunk id and a set of match reasons. Sort by accumulated score descending.

- [ ] **Step 4: Run to verify pass, regenerate the spec**

```bash
cargo test -p fubbik-api --test context_for_file
cargo run -- openapi > openapi.json
cargo test -p fubbik-api --test openapi
```

- [ ] **Step 5: Mutation-test the bonus ordering**

Set the file-ref bonus from 20 to 1. `file_ref_match_outranks_applies_to_match` must FAIL. Revert and report the output.

- [ ] **Step 6: Commit**

```bash
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api openapi.json .sqlx
git commit -m "feat(api): port GET /api/context/for-file

Five strategies with additive bonuses: file-ref +20, applies-to +10,
dependency +3, semantic +5, connected +2. A test pins that a file-ref
match outranks an applies-to match, which is what proves the bonuses are
ordered rather than merely present."
```

---

## Task 8: Context snapshots

**Files:**
- Create: `crates/fubbik-api/src/context/snapshot.rs`
- Modify: `crates/fubbik-api/src/context/{mod,routes}.rs`, `crates/fubbik-api/src/openapi.rs`
- Test: `crates/fubbik-api/tests/context.rs`

**Interfaces:**
- Consumes: Task 6's context service
- Produces: `POST /api/context/snapshot`, `GET /api/context/snapshot/{id}`, `GET /api/context/snapshots`, `DELETE /api/context/snapshot/{id}`

**Read `packages/api/src/context/snapshot-service.ts` (86 LOC) and `snapshot-routes.ts` (63 LOC).** Snapshots are frozen context persisted as JSONB and are **user-scoped**.

- [ ] **Step 1: Write the failing tests**

Write each out fully:

- `snapshot_round_trips_its_frozen_content` — create then retrieve, assert the content matches
- `snapshot_retrieval_is_user_scoped` — **user B retrieving user A's snapshot gets 404, and A's snapshot is still retrievable by A afterwards.** The second half matters: it proves the request was rejected rather than the row deleted.
- `snapshot_deletion_is_user_scoped` — user B deleting user A's snapshot gets 404 and A's snapshot survives
- `snapshots_list_only_returns_the_callers_own`

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-api --test context
```

Expected: the four new tests FAIL with 404.

- [ ] **Step 3: Implement**

Every read and delete filters on `user_id`. This is the same class as the cross-user gap Phase 4b closed in `enrich` — the filter is the security property, so it gets a test rather than an assumption.

- [ ] **Step 4: Run to verify pass, regenerate the spec**

```bash
cargo test -p fubbik-api --test context
cargo run -- openapi > openapi.json
cargo test -p fubbik-api --test openapi
```

- [ ] **Step 5: Mutation-test the scoping**

Remove the `user_id` filter from the snapshot read. `snapshot_retrieval_is_user_scoped` must FAIL. Do the same for delete and `snapshot_deletion_is_user_scoped`. Report both outputs.

- [ ] **Step 6: Commit**

```bash
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api openapi.json .sqlx
git commit -m "feat(api): port context snapshots

Four routes over frozen JSONB context. Every read and delete filters on
user_id, and each filter has a mutation-tested guard — the same class as
the cross-user gap Phase 4b closed in enrich."
```

---

## Task 9: `/chunks/export/context` and `/chunks/export/claude-md`

**Files:**
- Create: `crates/fubbik-api/src/context_export/{mod,service,claude_md,routes}.rs`
- Modify: `crates/fubbik-api/src/{lib,openapi}.rs`
- Test: `crates/fubbik-api/tests/context_export.rs` (new)

**Interfaces:**
- Consumes: `fubbik_core::{score, format, tokens}`
- Produces: two registered routes

**Read `packages/api/src/context-export/service.ts` (87 LOC) and `packages/api/src/context/claude-md.ts` (190 LOC).** CLAUDE.md generation is tag-based export with requirements and active plans, defaulting to a 32000-token budget.

- [ ] **Step 1: Write the failing tests**

Write each out fully:

- `export_context_respects_max_tokens` — assert a small budget yields strictly fewer chunks than a large one
- `export_context_boosts_chunks_relevant_to_for_path` — with `forPath` set, a chunk referencing that path outranks one that does not
- `claude_md_includes_tagged_chunks_and_excludes_untagged`
- `claude_md_defaults_to_a_32000_token_budget` — assert the default is applied when `maxTokens` is absent, by seeding enough content that the default visibly truncates
- `claude_md_includes_active_plans_and_requirements`

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-api --test context_export
```

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run to verify pass, regenerate the spec**

```bash
cargo test -p fubbik-api --test context_export
cargo run -- openapi > openapi.json
cargo test -p fubbik-api --test openapi
```

- [ ] **Step 5: Mutation-test the default budget**

Change the CLAUDE.md default from 32000 to `usize::MAX`. `claude_md_defaults_to_a_32000_token_budget` must FAIL. Revert and report the output.

- [ ] **Step 6: Commit**

```bash
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api openapi.json .sqlx
git commit -m "feat(api): port context export and CLAUDE.md generation

Tag-based export with requirements and active plans, defaulting to a
32000-token budget. The default is pinned by a test that seeds enough
content for it to visibly truncate."
```

---

## Task 10: `/spaces/{id}/generate-instructions`

**Files:**
- Create: `crates/fubbik-api/src/generate_instructions/{mod,service,routes}.rs`
- Modify: `crates/fubbik-api/src/{lib,openapi}.rs`
- Test: `crates/fubbik-api/tests/generate_instructions.rs` (new)

**Interfaces:**
- Consumes: Task 9's export service
- Produces: `GET /api/spaces/{id}/generate-instructions?format=claude|agents|cursor`

**Read `packages/api/src/generate-instructions/routes.ts`.** The CLI calls all three formats (`apps/cli/src/commands/generate.ts:35,58,81`), so all three must work.

- [ ] **Step 1: Write the failing tests**

Write each out fully:

- `each_format_produces_a_distinct_document` — assert `claude`, `agents` and `cursor` return **three different bodies**. Asserting each returns 200 would pass with all three producing identical output, which would silently break two of the CLI's three commands.
- `unknown_format_is_rejected_or_defaults` — match whatever `routes.ts` does; state which in a comment
- `generate_instructions_is_scoped_to_the_space_owner` — user B requesting user A's space gets 404

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-api --test generate_instructions
```

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run to verify pass, regenerate the spec**

```bash
cargo test -p fubbik-api --test generate_instructions
cargo run -- openapi > openapi.json
cargo test -p fubbik-api --test openapi
```

- [ ] **Step 5: Mutation-test the format branching**

Make all three formats return the `claude` document. `each_format_produces_a_distinct_document` must FAIL. Revert and report the output.

- [ ] **Step 6: Commit**

```bash
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api openapi.json .sqlx
git commit -m "feat(api): port /spaces/{id}/generate-instructions

All three formats the CLI calls — claude, agents and cursor — with a test
pinning that they produce distinct documents rather than three routes to
the same output."
```

---

## Task 11: Documentation correction and final verification

**Files:**
- Modify: `CLAUDE.md`

**Interfaces:**
- Consumes: every prior task
- Produces: the evidence that the spec's exit criteria are met

- [ ] **Step 1: Correct the tokenizer claim in CLAUDE.md**

Find the line describing token estimation as `cl200k_base`. `gpt-4o` maps to **`o200k_base`** — the claim is wrong. Correct it and note that Rust uses `tiktoken-rs` with the same encoding for exact parity.

- [ ] **Step 2: Full suite**

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
cargo test --workspace --no-fail-fast 2>&1 | grep -E "^test result:" | awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i}'
```

Expected: strictly more passed than Task 0's 1166, and still exactly 2 failed (the AGE pair). **Any third failure is a regression.**

- [ ] **Step 3: Lint, format, offline build**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
SQLX_OFFLINE=true cargo check --workspace --all-targets
```

Expected: all three exit 0.

- [ ] **Step 4: Web type-check**

```bash
pnpm run check-types
```

Expected: 7/7 successful, 0 errors. The generated client is unchanged by this slice — no web call site moves — so a failure here means something altered the API surface unexpectedly.

- [ ] **Step 5: Diff guards**

```bash
BASE=$(git merge-base main HEAD)
git diff --numstat "$BASE"...HEAD -- .sqlx | awk '{a+=$1; d+=$2} END {print "sqlx insertions="a" deletions="d}'
git diff "$BASE"...HEAD -- crates/fubbik-db/migrations | grep -E "^\+" | grep -vE "^\+\+\+|^\+\s*--|^\+\s*$" || echo "(no SQL added)"
```

Expected: `.sqlx` deletions **0**; no SQL added — this slice adds no schema.

- [ ] **Step 6: Confirm fubbik-core's shape**

```bash
ls crates/fubbik-core/src/
test ! -f crates/fubbik-api/src/chunks/health_score.rs && echo "health_score.rs correctly gone"
```

Expected: `error.rs format.rs health.rs lib.rs score.rs tokens.rs`, and the old path absent.

- [ ] **Step 7: Commit and report**

```bash
git add CLAUDE.md
git commit -m "docs: correct the tokenizer encoding to o200k_base

CLAUDE.md described token estimation as cl200k_base. Node calls
encodingForModel(\"gpt-4o\"), which maps to o200k_base; Rust uses
tiktoken-rs with the same encoding for exact parity."
```

Report the final counts against Task 0's baseline, every mutation result collected along the way, which tests executed versus skipped, and any divergence from Node introduced and why.

---

## Self-Review

**Spec coverage.** Every spec section maps to a task: `fubbik-core`'s new job → Tasks 1-4; tokenization → Task 2; the `health_score` move and its `openapi.json` guard → Task 1; `scoreChunk`/`budgetChunks` semantics → Task 3; resolvers and enrichment → Task 5; the four `context/*` endpoints → Tasks 6-8; `context-export` and CLAUDE.md generation → Task 9; `generate-instructions` → Task 10; the `cl200k_base` correction and exit criteria → Task 11. The three non-negotiable assertions the spec names — budgeter skips rather than truncates, scorer does not double-count freshness, snapshots are user-scoped — are Tasks 3, 3 and 8 respectively, each with a mutation.

**Deliberate omissions, each recorded in the spec:** `import-docs` (SSE, own slice), `budgetChunksWithCoverage` (zero callers), Node's `char/4` tokenizer fallback (cannot trigger under `tiktoken-rs`).

**Known soft spots.** Tasks 5 through 10 specify their tests by name and assertion rather than as complete code, because the Node sources they port run 87-332 LOC each and writing speculative Rust against them would produce fiction. Each task names its file, its exact test list, the Node source to read first, and a mutation that must fail — but the implementer writes the test bodies. **Task 5 contains one `todo!()` stub, which is illustrative of the helper set only and must not survive into the commit.** This is the one place this plan knowingly falls short of "no placeholders", and the reviewer should verify each named test exists and asserts what its name claims.

**Type consistency.** `ScoredChunk` is defined once in Task 3 and consumed unchanged by Tasks 4-9. `ChunkWithMetadata` wraps rather than extends it, because Rust has no interface inheritance — Node's `extends ScoredChunk` becomes a `chunk: ScoredChunk` field, and every later reference uses `c.chunk.title` rather than `c.title`.
