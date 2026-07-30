# Rust Backend & CLI Rewrite — Design

**Date:** 2026-07-28
**Status:** Approved design, pending implementation plan

## Goal

Rewrite the fubbik backend and CLI in Rust, driven by three objectives stated in priority-neutral order:

1. **Single-binary distribution** — ship fubbik as one static binary with no Node or Bun runtime required.
2. **Performance and memory** — reduce cost of graph sync, embedding, indexing, and context assembly.
3. **Correctness and maintainability** — replace the Effect/TypeScript service layer with compiler-enforced error handling.

The web frontend stays TypeScript. The VS Code extension is untouched.

## Scope

### In scope

| Surface | Current | LOC |
| --- | --- | --- |
| `packages/api` | Elysia routes, Effect services, 48 route modules | 18,038 |
| `packages/db` | Drizzle schema (37 files), repositories, AGE layer, seed | 18,513 |
| `packages/auth` | better-auth, email/password only | 27 |
| `apps/server` | Thin Elysia wrapper | 243 |
| `apps/cli` | 60 Commander.js command files | 10,451 |
| `packages/mcp` | MCP server on the TS SDK | ~1,532 |

### Out of scope

- `apps/web` (48,037 LOC) stays TanStack Start in TypeScript. It changes only where it must: the API client, the auth client, and the removal of SSR.
- `apps/vscode` is pure HTTP with no workspace imports. Untouched.

### Constraints established

- **Only a local database exists.** No external self-hosted users. A one-off dump/restore and a password reset are acceptable. This removes any requirement for better-auth wire compatibility — the single largest risk category in the original framing.
- **Full CLI parity.** All 60 commands port; none are dropped.
- **Everything folds into the binary:** API, embedded web assets, MCP server, and background jobs.

## Architecture

### Crate layout

One cargo workspace, one shipped binary. The 48 route domains become modules within `fubbik-api`, not separate crates — per-domain crates would degrade compile times without buying isolation.

```
crates/
  fubbik-core/   domain types, AppError, scoring/budgeting, pure logic
  fubbik-db/     sqlx pool, migrations, repositories, AGE, pgvector
  fubbik-ai/     Ollama + OpenAI clients, embeddings, tokenization
  fubbik-api/    axum routers + services + OpenAPI (48 domain modules)
  fubbik-mcp/    rmcp server
  fubbik-cli/    clap commands (HTTP client)
  fubbik/        binary: serve | mcp | <60 commands>
web/             unchanged TS, built to static assets, embedded at compile time
```

### Binary surface

- `fubbik serve` — axum API, embedded SPA, and background jobs in one process.
- `fubbik mcp` — stdio MCP server.
- All other subcommands are CLI commands.

### Dependency choices

| Concern | Choice | Replaces |
| --- | --- | --- |
| HTTP | axum + tower-http | Elysia |
| Database | sqlx (Postgres) | Drizzle |
| Errors | thiserror | Effect tagged errors |
| Logging/tracing | tracing + tracing-opentelemetry | winston + OTEL setup |
| OpenAPI | utoipa | @elysiajs/swagger |
| Asset embedding | rust-embed | — |
| CLI | clap v4 derive, comfy-table, owo-colors | commander, cli-table3, picocolors |
| MCP | rmcp | @modelcontextprotocol/sdk |
| Tokenization | tiktoken-rs | js-tiktoken |
| Password hashing | argon2 | better-auth scrypt |
| OpenAI | async-openai | Vercel AI SDK |

Exact versions are pinned during Phase 1 rather than fixed in this document.

### CLI transport

The CLI remains an HTTP client, as it is today. The server stays the only process that touches the database. A small number of genuinely local commands (`init`, `hooks`, `doctor`) work offline.

This is deliberate: linking `fubbik-cli` directly against `fubbik-db` would be marginally faster but would destroy the ability to point the CLI at a remote fubbik instance.

## Data layer

### Schema ownership

Schema ownership moves from Drizzle to SQL migrations. `pg_dump --schema-only` of the current database becomes `0001_init.sql`, hand-cleaned. From that point `sqlx migrate` is the source of truth, and Drizzle is retired at cutover.

### Why sqlx over SeaORM or Diesel

Three reasons specific to this codebase:

- **AGE is already raw SQL.** Graph access runs through `SELECT * FROM cypher('knowledge', $$ ... $$) AS (v agtype)`. An ORM contributes nothing here. The existing `escCypher` escaping and `ageAvailable` probe port over nearly verbatim.
- **pgvector and pg_trgm queries are raw SQL** for the same reason.
- **`sqlx::query_as!` checks every query against the real schema at compile time.** This is the concrete form the correctness objective takes: a mistyped column becomes a build error instead of a runtime 500.

The cost is that compile-time checking requires either a reachable database or a checked-in offline cache. The `.sqlx` cache is committed and regenerated with `cargo sqlx prepare`, so CI and fresh clones build without Postgres.

### AGE connection setup

AGE requires `LOAD 'age'` and a `search_path` adjustment per connection. This moves into sqlx's `after_connect` pool hook — cleaner than the current per-query handling.

### Repositories

Repositories keep their present shape: one module per entity, returning `Result<T, DbError>`.

## Error model

The Effect stack translates almost one-to-one, because tagged errors are what Rust enums natively are.

| Today | Rust |
| --- | --- |
| `Effect<T, DatabaseError>` | `Result<T, AppError>` |
| `.pipe(Effect.flatMap(...))` | `?` |
| `_tag` → HTTP status in global `.onError` | `impl IntoResponse for AppError` |
| `Effect.runPromise` in every route | nothing — handlers return `Result` |

`AppError` is a single `thiserror` enum with variants `Database`, `NotFound`, `Auth`, `Validation`, `Conflict`, and `External`, mapping to the same 400/401/404/500 codes in use today.

This eliminates a specific existing problem: `apps/server/src/index.ts:56` currently works around `FiberFailure` erasing errors into the literal string `"An error has occurred"`. In Rust the error value is the error.

## Authentication

Clean slate, which makes auth simpler rather than merely different. Email and password only, no OAuth, no email verification — so better-auth's four tables collapse to two:

- `user`, with a `password_hash` column
- `session`

The `account` and `verification` tables are dropped.

Implementation: argon2id hashing, a server-side session row keyed by a signed httpOnly cookie, and a `CurrentUser` axum extractor mirroring today's `requireSession(ctx)`.

This is hand-rolled (~150 LOC) rather than delegated to `tower-sessions`. For a local-first tool, owning the session table is preferable to inheriting a schema we do not control.

The `FUBBIK_IMPLICIT_DEV_SESSION` escape hatch carries over unchanged.

Web-side cost is small: only `apps/web/src/lib/auth-client.ts` touches better-auth.

## API contract and the web client

`utoipa` derive macros on handlers produce `openapi.json`, which keeps the existing `/docs` Swagger UI working. `openapi-typescript` turns that into TypeScript types.

### The call-site problem

`apps/web` currently gets end-to-end types from `treaty<Api>(...)`, which Rust cannot produce. The blast radius is **106 files containing 310 `api.*` call sites**.

Two options were considered:

- **Rewrite the call sites** to `openapi-fetch`'s `client.GET("/api/tags")` style. Idiomatic and well-trodden, but requires a mechanical pass over 106 files landing in a single cutover commit.
- **Preserve the call shape (chosen).** Eden's `api.api.tags.get()` is path-segments-as-properties with the method as the final call. That is reproducible with a JS `Proxy` plus a recursive mapped type over the generated OpenAPI paths.

### Decision

Preserve the call shape. One new file of roughly 200–300 LOC, mostly types, and the other 106 files never change — including `use-api-query` and `use-api-mutation`, which already funnel most usage.

The acknowledged trade-off: this concentrates non-trivial conditional-type work in one file, which sits in tension with the goal of leaving type gymnastics behind. It is accepted because the complexity is contained in one reviewable file rather than spread across 310 edited call sites, and because it keeps the cutover commit small enough to verify.

## Web frontend changes

SSR is dropped; the SPA is built to static assets and embedded via `rust-embed`. SSR coupling is minimal — **one server function** (`src/functions/get-user.ts`) and **two route loaders**.

Required web changes, in full:

1. Replace `src/utils/api.ts` (Eden treaty) with the generated-types proxy client.
2. Replace `src/lib/auth-client.ts` (better-auth client) with plain fetch against the new auth routes.
3. Remove `src/entry-server.ts` and migrate the one server function and two loaders to client-side fetching.
4. Switch the Vite build target to static output.

## Migration strategy

The Rust binary runs on its own port against its own database (`fubbik_rs`). The Node stack stays untouched and working on `main` for the entire migration.

This avoids two traps:

- **No shared-schema contention.** Two schema owners against one database would be unmanageable.
- **No dual-auth bridge.** Because auth is a clean slate, a strangler-fig proxy would require Rust-issued and better-auth sessions to be mutually honored by both servers during overlap — throwaway scaffolding that ships nothing.

A strangler-fig proxy only pays for itself when downtime is unaffordable. With a local-only database it buys nothing.

Cutover is a one-off `pg_dump`/restore into the new schema plus a password reset.

## Testing

The existing 454 test cases across 60 files are the parity oracle. **Tests port alongside their domain**, never as a cleanup phase.

- `#[sqlx::test]` gives each test a freshly migrated database inside a transaction that auto-rolls back.
- Route tests use `tower::ServiceExt::oneshot` against the `Router` directly — no server boot, no port binding.
- **`openapi.json` is checked in**, and CI fails if regeneration produces a diff. This is what prevents the API contract drifting out from under the generated TypeScript client.
- **A differential harness** runs both stacks simultaneously on different ports against identically seeded databases, firing the same request at each and diffing the JSON. This converts per-domain parity from a judgment call into a test run.

## Phase roadmap

This document covers whole-system architecture. Each phase receives its own implementation plan; the immediate next step produces the Phase 1 plan only.

### Phase 1 — Walking skeleton

Workspace and seven crates, `0001_init.sql`, `AppError`, auth, the utoipa-to-TypeScript proxy client, `rust-embed` plus `fubbik serve`, and the differential harness.

The **chunks** domain ports end-to-end, chosen deliberately as the hardest: 432-line routes, version history, tags, applies-to, and file-refs. Bulk import/export and semantic search are excluded here — they belong to Phases 4 and 5 — so Phase 1 covers chunk CRUD, list/filter/sort, version history, and the sub-resource endpoints.

Five CLI commands land in Phase 1, chosen to exercise the client end to end: `add`, `get`, `list`, `search`, and `health`. Phase 1 `search` is text search over the chunks list endpoint; the semantic variant arrives with embeddings in Phase 4.

Additionally, an **AGE spike**: one cypher round-trip and an `agtype` parse. Rust has no typed AGE driver, so `agtype` returns as a string requiring a hand-written parser. This is the sharpest unknown in the project, and deferring it to Phase 4 would defeat the purpose of building a skeleton first.

**Exit criteria:** the web app runs against Rust for chunks, login works, and the binary is self-contained.

### Phase 2 — Core domains

spaces, workspaces, tags, tag-types, connections, templates, collections, favorites, activity, settings, scope-keys, use-cases, vocabulary, comments, notifications. Predominantly CRUD; fast once Phase 1 establishes the patterns.

### Phase 3 — Planning domains

requirements, plans, tasks, features (delta overlays), proposals, matrices, coverage, learning-paths. Real business logic. Feature-delta resolution and matrix cell-status computation are the two areas warranting care.

### Phase 4 — Knowledge intelligence

The context pipeline (resolvers, scorer, budgeter, formatter, CLAUDE.md generation, snapshots), search, embeddings and enrichment, staleness, knowledge-health, density, code-index, diagram, generate-instructions, timeline, concepts, graph and AGE sync, saved-graphs, stats, usage. The largest and most algorithmic phase.

### Phase 5 — CLI parity, MCP, background jobs

CLI commands land with their domain where practical; this phase sweeps up the remainder of the 60, ports `packages/mcp` to `rmcp`, and moves scheduled scans (staleness, graph sync, embedding refresh) into tokio.

### Phase 6 — Cutover

`pg_dump`/restore, flip the web environment variable, delete the TypeScript backend packages, update Docker and CLAUDE.md, cross-compile release binaries for macOS, Linux, and Windows.

## Risks

| Risk | Mitigation |
| --- | --- |
| `agtype` parsing has no typed Rust driver | Spiked in Phase 1, not deferred to Phase 4 |
| sqlx offline-cache workflow friction | `.sqlx` committed; `cargo sqlx prepare` in CI |
| utoipa may not cover every response shape | Checked-in `openapi.json` with CI diff detection |
| Binary size ~40–70MB with SPA embedded | Accepted; acceptable for the distribution goal |
| Domain porting drifts from Node behavior | Differential harness from Phase 1 onward |
| Cross-compilation for three platforms | Deferred to Phase 6; not on the critical path |

### AGE spike outcome (Task 3, Phase 1)

The `agtype` parsing risk above was spiked against a live AGE 1.7.0 instance
(`fubbik-rs-db`, Docker, Postgres 18) via `crates/fubbik-db/src/age.rs` and
`crates/fubbik-db/tests/age.rs`. The round-trip test (`cypher_round_trips_a_real_vertex`)
ran against real agtype output — AGE was available in the `#[sqlx::test]`-provisioned
database, so the skip branch was not exercised. All four points below were confirmed
directly, not assumed:

- **Confirmed.** `v::text` is unusable: `SELECT v::text ...` raises
  `agtype_value_to_text: unsupported argument agtype 6` for vertex, edge, and path
  values (reproduced against a `CREATE (n:chunk {...}) RETURN n` result).
- **Confirmed.** `agtype_out(v)` returns pseudo-type `cstring`; this cannot be
  selected into a result column at all (Postgres cannot materialise `cstring` into
  a table), so sqlx never even gets a chance to fail decoding it — the query itself
  is rejected.
- **Confirmed.** `v::varchar` is the working extraction, verified across both a
  real vertex (`cypher_round_trips_a_real_vertex`) and a bare scalar
  (`cypher_returns_scalars`, `RETURN 42`).
- **Confirmed.** Suffix stripping is bounded to a trailing `::identifier` (all
  lowercase ASCII after the last `::`, non-empty). `preserves_property_values_containing_double_colons`
  proves a property value of `"a::b"` survives both the AGE round-trip and the
  parser unchanged.

**One shape the brief's code did not anticipate, discovered during the spike:**
AGE's `cypher()` function is unresolvable on a connection that has not run
`LOAD 'age'` in that session — this is true even when the call is fully
schema-qualified as `ag_catalog.cypher(...)`, which still fails with
`unhandled cypher(cstring) function call`. `fubbik_db::connect()`'s
`after_connect` hook does run `LOAD 'age'` and set `search_path`, but
`#[sqlx::test]`-provisioned pools (used by this spike's own integration tests,
and by all future `fubbik-db` tests) bypass `connect()` entirely and get bare
pooled connections. `age::cypher()` was therefore changed from the brief's
`pool.fetch_all(...)` to acquire a single connection, run `LOAD 'age';` and
`SET search_path = ag_catalog, "$user", public;` on it, and run the query on
that same connection — making `cypher()` self-sufficient regardless of how
the pool was constructed, rather than relying on every caller to have gone
through `connect()` first. Phase 4 should keep this connection-priming inside
`age::cypher()` rather than pushing the requirement onto callers.

**Correction (post-review): nested composites DID defeat the original parser.**
The first pass of this spike only tested vertices and bare scalars, and
concluded — wrongly — that no agtype shape defeated the parser. It never
tried an edge or a real path query. A real path result from AGE 1.7.0 looks
like:

```
[{"id":...,"label":"probe_rev",...}::vertex, {"id":...,"label":"REL_REV",...}::edge, {"id":...,"label":"probe_rev",...}::vertex]::path
```

Every vertex/edge nested inside the array carries its OWN `::vertex`/`::edge`
suffix in addition to the outer `::path` suffix. The original `parse_agtype`
only stripped a single *trailing* `::identifier`, so it left the inner
`::vertex`/`::edge` markers embedded in what it then tried to hand to
`serde_json::from_str`. That is not valid JSON, parsing failed,
`parse_agtype` returned `None`, and `cypher()`'s `filter_map` silently
dropped the row — no error, no log, nothing to indicate a path query had
effectively returned zero results. Phase 4's graph path-finding work depends
on exactly this shape, so this was a real, load-bearing gap, not a
theoretical one.

The fix (`crates/fubbik-db/src/age.rs`) replaces trailing-suffix stripping
with `strip_type_suffixes`, a small scanner that walks the whole agtype
string, tracks whether it's inside a double-quoted JSON string (respecting
backslash escapes so an escaped quote can't wrongly end a string early), and
removes every `::` followed by one or more ASCII lowercase letters that
occurs *outside* a string — regardless of nesting depth or how many such
suffixes appear. This uniformly handles vertices, edges, paths of arbitrary
nesting, and scalar suffixes (e.g. `1.5::numeric`), while still leaving `::`
sequences inside JSON string values (e.g. `{"code": "a::b"}`) untouched.

Coverage added for this fix:
- Unit tests in `age.rs`: `strips_edge_suffix`, `strips_all_nested_suffixes_in_a_path`
  (a hand-built 3-element vertex+edge+vertex path literal), `strips_numeric_scalar_suffix`,
  plus the pre-existing vertex/scalar/`::`-in-string-property cases, all still passing.
- Integration tests in `tests/age.rs` run against real AGE 1.7.0, not just hardcoded
  literals: `cypher_round_trips_a_real_edge` (creates and matches a real edge, asserts
  a `::`-bearing edge property survives) and `cypher_round_trips_a_real_path` (creates
  a real `(a)-[r]->(b)` pattern, runs `MATCH p = (a)-[r]->(b) RETURN p`, and asserts the
  row is not dropped and unmarshals into a 3-element `[vertex, edge, vertex]` array with
  the expected labels).

`cypher()` also now emits `tracing::warn!` (including the raw offending string)
whenever `parse_agtype` returns `None`, so a future parser gap fails loudly via
logs instead of silently vanishing the way this one did.

Also newly documented: `cypher(pool, query)` interpolates the caller-supplied
`query` directly into a `$$`-dollar-quoted SQL statement. `esc_cypher` only
escapes `\` and `'` for Cypher string-literal safety — it does not protect
the `$$` SQL delimiter, so a value containing the literal substring `$$`
could terminate the dollar-quoting early and inject SQL. This is inherited
unchanged from the TypeScript original (`packages/db/src/age/client.ts`), not
newly introduced here, and is called out as a `# Safety` doc comment on
`cypher()` rather than re-architected in this fix.

## Explicitly rejected

- **Strangler-fig proxy migration** — requires a throwaway dual-auth bridge for no benefit given local-only data.
- **Keeping SSR** — would make "single binary" mean "two processes" and force Node into the Docker image.
- **Porting only hot paths** — inconsistent with the correctness and distribution objectives, which both require the whole backend.
- **Rewriting the 310 call sites** — larger cutover commit, more review surface, no lasting advantage.
- **SeaORM or Diesel** — AGE, pgvector, and pg_trgm all need raw SQL regardless.
- **Trimming the CLI surface** — considered and declined; full parity for all 60 commands.
