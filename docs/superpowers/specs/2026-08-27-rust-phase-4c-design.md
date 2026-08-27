# Rust Rewrite Phase 4c — Context Core

**Date:** 2026-08-27
**Status:** Approved design
**Follows:** `2026-08-27-rust-phase-4b-design.md` (Ollama core)
**Part of:** Phase 4 (knowledge intelligence), decomposed into 4a / 4b / 4c

## What changed since 4a scoped this slice

The 4a spec defined 4c as "`fubbik-core`'s scorer/budgeter/formatter, `context/*`,
`context-export`, CLAUDE.md generation, `generate-instructions`, `import-docs`." Measuring
that list before planning changed it in three ways.

**`import-docs` is a different class of work and is deferred.** Its web client reads
`response.body.getReader()` and parses `data:` lines (`apps/web/src/features/import/use-sse-import.ts:41-58`)
— it is Server-Sent Events. No streaming response exists anywhere in the Rust port today.
Porting it means introducing the port's first streaming endpoint, which is a different
problem from the scoring-and-formatting work that makes up the rest of this slice. It gets
its own slice, for the same reason OpenAI's `ai/*` was split out of 4b: one dependency, one
test strategy per slice.

**`budgetChunksWithCoverage` is dead and will not be ported.** `packages/api/src/context/utils.ts:81`
has **zero callers**. Its only source of `communityId` is
`packages/api/src/graph/community-analysis.ts`, which Phase 4a already established has no
clients in any consumer. Porting it would mean writing untestable code for a code path that
cannot execute. The sibling `budgetChunks` is the one every caller uses.

**This slice moves almost no consumer onto Rust.** Unlike 4a (four web call sites) and 4b
(five), 4c's endpoints are consumed overwhelmingly by `apps/cli` and `packages/mcp`, both of
which are still TypeScript until Phase 5. The work lands and is testable, but nothing
visibly switches over. That is expected, not a gap — it is worth stating so the absence of a
"migrate the callers" task is not read as an oversight.

## Scope

Every endpoint below has a real caller, measured by grepping `apps/web/src`, `apps/cli/src`,
`apps/vscode/src` and `packages/mcp/src`:

| Endpoint | Callers today |
| --- | --- |
| `GET /api/context/for-file` | `apps/cli/src/commands/{context-for,watch,gaps}.ts` |
| `GET /api/context/for-plan` | `apps/cli/src/commands/context-for-plan.ts`, `packages/mcp/src/context-tools.ts` (×3) |
| `GET /api/context/about` | `apps/cli/src/commands/context-about.ts`, `packages/mcp/src/context-tools.ts` |
| `GET /api/context/for-files` | `apps/cli/src/commands/{context-for,context-for-diff,context-dir}.ts`, `packages/mcp/src/context-tools.ts` |
| `POST/GET/DELETE /api/context/snapshot`, `GET /api/context/snapshots` | `apps/cli/src/commands/context-snapshot.ts` |
| `GET /api/chunks/export/context` | `apps/cli/src/commands/context.ts` |
| `GET /api/chunks/export/claude-md` | `apps/cli/src/commands/sync-claude-md.ts`, `packages/mcp/src/context-tools.ts` |
| `GET /api/spaces/{id}/generate-instructions` | `apps/cli/src/commands/generate.ts` (×3 formats) |

**Out of scope, deliberately:** `import-docs` and `import-docs/preview` (SSE — own slice),
`budgetChunksWithCoverage` (dead), and everything the earlier phases already excluded.

## `fubbik-core` gets its real job

`crates/fubbik-core/src/` is currently 54 lines — `error.rs` and a one-line `lib.rs`. This
slice makes it the home for pure knowledge-scoring logic: no database, no HTTP, no Ollama,
so every function in it is testable with plain unit tests and no fixtures.

It gains:

- `tokens.rs` — `estimate_tokens(text: &str) -> usize`
- `score.rs` — `score_chunk(...) -> f64`, `budget_chunks(...)`
- `format.rs` — `format_chunk_text(...)`
- `health.rs` — **moved** from `crates/fubbik-api/src/chunks/health_score.rs`

**The move is deliberate.** `score_chunk` calls `compute_health_score`, so leaving health
scoring in `fubbik-api` would point a dependency the wrong way. It is pure logic that never
touched the database, so it belongs beside the scorer. `fubbik-api` keeps working by
importing it; its one existing caller at `crates/fubbik-api/src/chunks/service.rs:537` is
updated. This is churn inside a port, which the plan should minimise elsewhere — it is
accepted here because the alternative is a permanently inverted dependency.

## Tokenization — exact parity, via a new dependency

Node calls `encodingForModel("gpt-4o")` from `js-tiktoken` and falls back to
`Math.ceil(text.length / 4)` only if the encoder fails to load
(`packages/api/src/context/utils.ts:19-37`).

Rust uses the `tiktoken-rs` crate with **`o200k_base`** — the encoding `gpt-4o` maps to.

This matters more than a dependency choice usually does. `budget_chunks` greedily fills a
token budget, so the tokenizer decides **which chunks appear in an export**. A `char/4`
approximation would under-count code-heavy content, over-fill budgets, and select a
different set of chunks than Node for the same request — a divergence no test could catch
without comparing the two backends directly. Exact parity is the only option under which a
differential test between Node and Rust can pass.

**Two documentation corrections fall out of this.** `CLAUDE.md` describes the tokenizer as
`cl200k_base`; `gpt-4o` maps to `o200k_base`, so that line is wrong and should be fixed.
And Node's `text.length` is UTF-16 code units, which matches neither Rust's `len()` (bytes)
nor `chars().count()` (Unicode scalar values) for non-ASCII text — relevant only if the
fallback path is ever ported, which it is not.

**The fallback is deliberately not ported.** Node's exists because `js-tiktoken` loads its
BPE data lazily and can fail; `tiktoken-rs` embeds its data at compile time and cannot fail
the same way. Porting a fallback that can never trigger would add an untestable branch whose
only effect, if it somehow fired, would be to silently change export contents.

## Data flow

**`scoreChunk`** (`utils.ts:39-61`) is a sum of five terms, and the comment on the last one
is load-bearing: health contributes `health.total / 10`, and there is **no separate freshness
term** because freshness is already inside `computeHealthScore`. Adding one would double-count
it. Type scores 3 for `document`, 1 for `note`, 2 otherwise; rationale scores 2 when present;
connections score `min(count * 2, 10)`; review status scores 2 for `approved`, 1 for
`reviewed`, 0 otherwise.

**`budgetChunks`** (`utils.ts:63-76`) sorts by score descending, seeds the running total with
`estimate_tokens("# Project Context\n\n")`, and **skips** any chunk that would exceed the
budget rather than stopping. That distinction is behavioural: a single oversized chunk does
not truncate the export, it is passed over while smaller lower-scored chunks still fit.

**`context/for-file`** (`context-for-file/service.ts`) combines five strategies with additive
bonuses — file-ref (+20), applies-to (+10), dependency (+3), semantic (+5, requires Ollama),
connected (+2) — each tagging its results with a `matchReason`. The semantic strategy uses
Phase 4b's `embed_query` and `semantic_search`, capped at 10 results.

**Snapshots** are frozen context persisted as JSONB and are **user-scoped**: retrieval and
deletion must check ownership. This is the same class as the cross-user gap Phase 4b closed
in `enrich`, and it needs its own test rather than an assumption.

## Testing

Everything in `fubbik-core` is pure and gets plain unit tests with no database.

The endpoints get `#[sqlx::test]` integration tests, and where they depend on embeddings they
use the `wiremock` pattern Phase 4b established — mock `/api/embeddings`, seed one-hot
768-dimension vectors directly in SQL so cosine distance is exactly 0 or 1 and expected
orderings are unambiguous.

**Three specific assertions this slice must not ship without:**

- **The budgeter skips rather than truncates.** Seed one oversized chunk with the highest
  score plus smaller ones that fit; assert the smaller ones are still selected. A test that
  only checks "the result fits in the budget" passes with a `break` instead of a `continue`.
- **The scorer does not double-count freshness.** Two chunks identical but for `updated_at`
  should differ by exactly the health-derived amount. A test asserting only "fresher scores
  higher" would pass with a freshness term added on top.
- **Snapshot ownership.** User B retrieving user A's snapshot gets 404, and A's snapshot is
  unaffected.

**Token parity is worth pinning directly.** `estimate_tokens` should be asserted against
known-good counts for a handful of fixed strings, so a future `tiktoken-rs` upgrade that
changes the encoding is caught by a red test rather than by exports quietly changing shape.

## Risks

**The health-score move touches shipped code.** It is mechanical, but it is the only part of
this slice that can break something already working. It should be its own task, landing
before anything depends on it, so a failure there is not tangled with new behaviour.

**`tiktoken-rs` bundles BPE merge data**, adding several megabytes to the build and some
first-call initialisation cost. Acceptable for exact parity, but worth measuring rather than
assuming — the plan should note the build-size delta.

**The disk is the immediate practical constraint.** The machine sat at 948Mi free
(100% capacity) when this spec was written, and a new dependency plus a moved module means a
substantial rebuild. Implementation cannot start until that is cleared.

**Phase 4b's AGE-dependent paths remain unverified.** Two tests fail by design without Apache
AGE and three skip silently while reporting as passed. 4c does not depend on them, but the
branch it builds on carries that gap.

## Exit criteria

- `cargo test --workspace --no-fail-fast` passes with no reduction against Phase 4b's
  1166 passed / 2 failed / 13 ignored
- `cargo clippy --workspace --all-targets -- -D warnings` exits 0
- `cargo fmt --check` clean
- `SQLX_OFFLINE=true cargo check --workspace --all-targets` exits 0
- `pnpm run check-types` passes
- `.sqlx` changes are additive
- `openapi.json` regenerated and confined to the new routes
- `crates/fubbik-core/src/` contains the scorer, budgeter, formatter, tokenizer and health
  score, and `crates/fubbik-api/src/chunks/health_score.rs` no longer exists
- `CLAUDE.md`'s `cl200k_base` claim is corrected to `o200k_base`
