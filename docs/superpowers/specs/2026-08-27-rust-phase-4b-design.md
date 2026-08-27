# Rust Rewrite Phase 4b — Ollama Core

**Date:** 2026-08-27
**Status:** Approved design
**Follows:** `2026-08-26-rust-phase-4a-design.md` (Graph & AGE)
**Part of:** Phase 4 (knowledge intelligence), decomposed into 4a / 4b / 4c

## What changed since 4a scoped this slice

The 4a spec defined 4b as "the `fubbik-ai` crate (Ollama + OpenAI clients, the embedding
*write* path, tokenization), then `enrich`, `ai/*`, `search/semantic`, `check-similar`,
`{id}/neighbors`, `{id}/suggestions`, `clusters`, `grouped`, `search/federated`." Measuring
that list before planning changed its shape three ways.

**Rust already calls Ollama.** `crates/fubbik-api/src/vocabulary/suggest.rs` is a working
Ollama client path shipped with the vocabulary domain, and `reqwest` is already a
`fubbik-api` dependency. `fubbik-ai` is an extraction with an existing caller, not
greenfield.

**Four of the ten listed endpoints are not AI at all.** `clusters`
(`packages/db/src/repository/clusters.ts`, pure SQL), `{id}/suggestions`
(`packages/api/src/chunks/suggestions.ts` — shared tags plus title similarity, no
embeddings), `grouped` (`packages/db/src/repository/chunk-groups.ts`, ~300 LOC of SQL) and
`search/federated` (`packages/api/src/chunks/federated-search.ts`, plain SQL) were bucketed
into 4b by file proximity, not by dependency. They need porting; nothing about them needs
the AI crate to exist first.

**`ai/*` is OpenAI, not Ollama.** `packages/api/src/ai/service.ts` uses `@ai-sdk/openai`
with `OPENAI_API_KEY` — a different client, a different failure mode and a different test
strategy from everything else on the list.

So this slice is **Ollama core only**: one dependency, one test strategy, one coherent unit.
`ai/*` (OpenAI) and the four pure-SQL endpoints get their own slices.

## What this closes

Two live defects, not just unported routes.

**Rust never refreshes embeddings.** Node fires a full re-enrich on every title/content edit
(`chunk-mutations.ts:215`). Rust's chunk update does nothing of the kind, so after cutover
every edited chunk would silently keep a stale vector and semantic search would decay
against edits it never saw.

**Rust's unified search has a stubbed semantic branch.** `crates/fubbik-api/src/search/service.rs:399`
documents that its `"semantic"` result is a placeholder because "no embedding/Ollama
pipeline exists anywhere in this Rust port." That comment stops being true in this slice.

## Scope

Five endpoints, each with a real caller measured by grepping `apps/web/src`, `apps/cli/src`,
`apps/vscode/src` and `packages/mcp/src`:

| Endpoint | Callers today |
| --- | --- |
| `POST /api/chunks/{id}/enrich` | `apps/web/src/utils/api-helpers.ts:22`, `apps/web/src/routes/knowledge-health.tsx:285` |
| `POST /api/chunks/enrich-all` | `apps/cli/src/commands/enrich.ts:25` |
| `GET /api/chunks/search/semantic` | `apps/web/src/features/chunks/related-suggestions.tsx:25`, `apps/cli/src/commands/search.ts:53` |
| `POST /api/chunks/check-similar` | `apps/web/src/features/chunks/similar-chunks-warning.tsx:23` |
| `GET /api/chunks/{id}/neighbors` | `apps/web/src/features/chunks/detail/chunk-neighbors.tsx:30` |

Plus the fire-and-forget re-enrich on `PATCH /api/chunks/{id}`, and replacing the stubbed
`"semantic"` branch in unified search.

**Out of scope, deliberately:** OpenAI `ai/*` (`summarize`, `suggest-connections`,
`generate`), `ai/structure-requirement`, `clusters`, `{id}/suggestions`, `grouped`,
`search/federated`, `requirements/suggest-context`, `chunks/tag-suggestions`.

## The `fubbik-ai` crate

A dumb transport. It knows Ollama's HTTP contract and nothing about fubbik — no prompt text
lives in it, because prompts are domain knowledge that belongs with the domain that owns
them.

```rust
pub struct OllamaClient { base_url: String, http: reqwest::Client }

impl OllamaClient {
    pub fn new(base_url: impl Into<String>) -> Self;
    pub fn from_env() -> Self;                              // OLLAMA_URL, default http://localhost:11434
    pub async fn is_available(&self) -> bool;               // GET /api/tags, 2s timeout, never errors
    pub async fn generate_json<T: DeserializeOwned>(&self, prompt: &str, model: &str) -> Result<T, AiError>;
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, AiError>;
    pub async fn embed_query(&self, q: &str) -> Result<Vec<f32>, AiError>;
    pub async fn embed_document(&self, title: &str, summary: Option<&str>, content: &str) -> Result<Vec<f32>, AiError>;
}
```

`embed_query` and `embed_document` exist as distinct methods because `nomic-embed-text` is
asymmetric: Node prefixes queries with `search_query: ` and documents with
`search_document: ` (`packages/api/src/ollama/client.ts:66,71`). Dropping a prefix does not
fail — it silently degrades every similarity score — so the prefixes live in the crate with
a test asserting what goes on the wire, rather than being re-typed at each call site.

`embed_document` reproduces Node's exact construction:
`format!("search_document: {title}\n{summary_or_empty}\n{content}").trim()`.

**Errors.** `AiError` converts into `fubbik_core::AppError::External`, which
`crates/fubbik-api/src/error.rs:36` already maps to `502 BAD_GATEWAY` — the same status
Node's global handler gives `AiError` (`packages/api/src/index.ts:193`). No new error
variant is introduced.

**The existing caller moves.** `vocabulary/suggest.rs` is refactored onto
`generate_json`, keeping its own prompt and its infallible `unwrap_or_default()` wrapper
(which matches Node's `Effect.Effect<SuggestedEntry[], never>`). This is the proof the seam
is right: if the client cannot serve the one caller that already exists, the boundary is
wrong and we find out in the first task rather than the last.

## Injection, not environment lookup

`AppState` gains `pub ai: OllamaClient`, constructed once at startup via `from_env()`.

This is the load-bearing decision of the slice. Today `vocabulary/suggest.rs:53` resolves
`OLLAMA_URL` at call time, which is precisely why its only test
(`crates/fubbik-api/tests/vocabulary.rs:372`) can assert nothing but the degraded path — it
passes because no Ollama is running, and the success path has never been executed by any
test in this repository. With the client on state, every `#[sqlx::test]` points it at its own
`wiremock` server and the success paths become testable in CI, which has pgvector but no
Ollama.

Consequence: `suggest_vocabulary`'s `ollama_url: Option<&str>` override parameter is removed,
since the state-injected client supersedes it. That is a signature change to shipped code and
its existing test moves to the wiremock pattern.

## Data flow

**Enrich** (`packages/api/src/enrich/service.ts`). If `is_available()` is false, return
`Ok(None)` and write nothing — Node returns `null` here, and preserving that matters because
the CLI's `enrich-all` counts non-null results. Otherwise run `generate_json` for
`{summary, aliases, notAbout}` and `embed_document` concurrently via `tokio::try_join!`
(Node uses `Effect.all`), then perform one `update_chunk_enrichment` write.

**Semantic search** (`packages/api/src/chunks/chunk-search.ts:59-73`). `embed_query(q)` then
`repo::semantic::semantic_search`. `limit = min(limit.unwrap_or(5), 20)`; `exclude` is a CSV
of terms each filtered with `NOT (not_about @> '["term"]'::jsonb)`; `scope` is a CSV of
`key:value` pairs, pairs without exactly one colon discarded, filtered with `scope @> …::jsonb`.

**check-similar** (`packages/api/src/chunks/similarity.ts`). Unavailable → `[]`. Otherwise
`embed_document(title, None, content)` → `find_similar_by_embedding` with threshold 0.75 and
limit 3. Note the threshold: the repository function defaults to 0.7, but this call site
passes 0.75 explicitly.

**neighbors** (`packages/api/src/chunks/chunk-search.ts:31-45`). If the source chunk has no
embedding, return `{neighbors: [], note: "Chunk has no embedding — run enrichment first."}`
without touching Ollama. Otherwise `find_neighbors_by_chunk_id(k * 2)` combined with AGE
`get_neighborhood(chunk_id, 2)`: `combined_score = (1 - distance) + 0.15` when
graph-connected, sort descending, take `k`. AGE failure degrades to no bonus rather than
erroring, matching Node's `Effect.catchAll`. `k` is clamped to `1..=50` at the route.

**Re-enrich on edit** (`packages/api/src/chunks/chunk-mutations.ts:213-217`). When `PATCH`
changes `title` or `content`, spawn the full enrich detached and log failures. This
regenerates `summary`, `aliases` and `notAbout` as well as the embedding, overwriting a
hand-written summary. That overwrite is Node's actual behaviour and is reproduced exactly;
if it is unwanted it is a pre-existing product bug to fix deliberately in its own change,
not silently during a port.

**Vector writes.** `crates/fubbik-db/src/embedding.rs` decodes the column as text via
`embedding::text` rather than adding the `pgvector` crate. Writes follow the same approach:
bind `format!("[{}]", values.join(","))` as text and cast with `$n::text::vector` in SQL. No
new dependency, and the round-trip is covered by a test that writes a vector and reads it
back through the existing `EmbeddingVec` decoder.

## Rate limiting

Node rate-limits two of these endpoints through an in-memory fixed window
(`packages/api/src/middleware/rate-limit.ts`): enrich at 10 per 60s keyed `enrich:{userId}`,
semantic search at 30 per 60s keyed `semantic-search:{userId}`. Both respond 429 with
`{error: "Rate limit exceeded", retryAfter: <seconds>}`. Rust has no rate limiting anywhere
today.

`crates/fubbik-api/src/middleware/rate_limit.rs` reproduces this: a
`Mutex<HashMap<String, Window>>` held in `AppState`, wired to those two endpoints only —
not as a general axum layer, because no other endpoint asks for one. Node's 5-minute sweep
timer becomes opportunistic eviction on lookup; a background task for a bounded per-user map
is not worth its own moving part.

## Testing

CI runs `pgvector/pgvector:pg18` (`.github/workflows/rust.yml:17`) — pgvector is present, so
every vector-ranking assertion runs there. Ollama is not present and never will be, so
`wiremock` (already a `fubbik-cli` dev-dependency) becomes a dev-dependency of `fubbik-ai`
and `fubbik-api`.

**`fubbik-ai` unit tests.** For each method: the success path; a non-2xx response mapping to
`AiError`; a malformed JSON body mapping to `AiError`; `is_available` returning false both on
connection refused and on a 500. Plus the prefix assertions — that `embed_query("x")` puts
`search_query: x` on the wire, and that `embed_document` builds the trimmed
`search_document: …` string Node builds.

**`fubbik-api` integration tests** (`#[sqlx::test(migrations = "../fubbik-db/migrations")]`
plus a per-test wiremock server):

- enrich writes all four columns and returns them
- enrich with Ollama unavailable writes nothing and still returns 200
- semantic search ranking: two chunks seeded at known 768-dimension vectors and a mocked
  query vector, asserting the returned *order*, not merely the membership
- `exclude` and `scope` filters each drop a row that would otherwise rank first
- check-similar's 0.75 threshold excludes a chunk that sits just below it
- neighbors' no-embedding note, without any Ollama call
- the 0.15 graph bonus reorders two otherwise-tied neighbours (AGE-gated, skipped when AGE
  is absent, as in 4a)
- `PATCH` of title or content triggers the re-enrich; `PATCH` of another field does not
- the 11th enrich inside the window returns 429 with `retryAfter`
- unified search's `"semantic"` branch returns real hits rather than the stub

**Every threshold and limit test asserts the excluded side of the boundary.** A test that
checks only that a match above the threshold is returned passes with the threshold deleted
entirely. This is the vacuity class that cost a fix round in 4a.

## Risks

**Fire-and-forget is hard to assert.** The re-enrich is a detached task, so its test needs a
bounded poll for the written columns, not a fixed sleep. A sleep long enough to be reliable
is long enough to slow the suite, and a short one is flaky.

**`cargo sqlx prepare` must include tests.** Per the 4a constraint, `cargo sqlx prepare
--workspace` alone drops test-target-only entries; preparation must be
`cargo sqlx prepare --workspace -- --tests`. The new vector-typed queries make this
regression easy to hit.

**enrich-all is unbounded in a bad way.** Node caps at 1000 chunks with concurrency 3
(`packages/api/src/enrich/routes.ts:32-46`). Both numbers are matched exactly rather than
improved, so the port stays a port; changing them is a separate decision.

**`AppState` grows.** All 42 existing `AppState` construction sites gain a field; 35 of them
are the `fn state(pool)` helper duplicated across `crates/fubbik-api/tests/`. This is mechanical but wide, and is sequenced as its own task so a
failure there is not tangled with behavioural work.

## Exit criteria

- `cargo test --workspace` passes with no reduction against the Phase 4a baseline of
  1084 passed / 0 failed / 13 ignored
- No AI test skips: every wiremock-backed assertion runs in CI
- `cargo clippy --workspace --all-targets -- -D warnings` exits 0
- `cargo fmt --check` clean
- `SQLX_OFFLINE=true cargo check --workspace --all-targets` exits 0
- `pnpm run check-types` passes; the five migrated web call sites use `api`, not `legacyApi`
- `crates/fubbik-api/src/search/service.rs`'s "no Ollama pipeline exists" comments are gone
  and its `"semantic"` branch returns real results
- `.sqlx` changes are additive
