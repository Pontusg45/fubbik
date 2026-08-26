# Rust Rewrite Phase 4a — Graph & AGE

**Date:** 2026-08-26
**Status:** Approved design; plan at `docs/superpowers/plans/2026-08-26-rust-phase-4a.md`
**Follows:** `2026-08-12-rust-phase-2d-design.md` (migrating the web app)
**Part of:** Phase 4 (knowledge intelligence), decomposed into 4a / 4b / 4c

## Why Phase 4 is three slices

`2026-07-28-rust-backend-cli-rewrite-design.md` defines Phase 4 as "the context pipeline,
search, embeddings and enrichment, staleness, knowledge-health, density, code-index, diagram,
generate-instructions, timeline, concepts, graph and AGE sync, saved-graphs, stats, usage" and
calls it "the largest and most algorithmic phase." Since that was written, staleness,
knowledge-health, density, timeline, saved-graphs, stats and search have all landed on the
`rust-phase2e` branch. What remains is ~3,200 LOC of Node across fourteen domains, plus a
`fubbik-ai` crate that does not exist and the scoring/budgeting half of `fubbik-core` that does
not exist either (`crates/fubbik-core/src/` is `error.rs` and `lib.rs`).

That is too much for one plan. Phase 4 is split into three slices, each with its own spec, plan
and branch:

- **4a — Graph & AGE** (this document). No AI dependency. Reuses AGE primitives that already
  exist but that no route has ever called.
- **4b — AI foundation.** The `fubbik-ai` crate (Ollama + OpenAI clients, the embedding *write*
  path, tokenization), then `enrich`, `ai/*`, `search/semantic`, `check-similar`,
  `{id}/neighbors`, `{id}/suggestions`, `clusters`, `grouped`, `search/federated`.
- **4c — Context pipeline.** `fubbik-core`'s scorer/budgeter/formatter, `context/*`,
  `context-export`, CLAUDE.md generation, `generate-instructions`, `import-docs`.

4b precedes 4c because the context pipeline's semantic resolver needs embeddings. 4a is
independent of both, which is why it goes first: it is the cheapest slice that unblocks a whole
page, and it exercises 830 lines of AGE code before anything else comes to depend on it.

## What already exists

**`crates/fubbik-db/src/age.rs` — 830 lines, zero callers.** A Cypher runner, an `agtype`
parser, `ensure_vertex`, `create_edge`, `delete_edge`, `count_edges_between`,
`backfill_connections`, `get_neighborhood`, `find_shortest_path_with_details`,
`compute_impact_ripple`. Written in Phase 1 as the AGE spike and covered by `tests/age.rs`, but
**no route in `fubbik-api` calls any of it.** This slice is the first consumer.

**`crates/fubbik-db/src/embedding.rs`.** Decodes the pgvector column as text. Read-only; there
is no write path. That belongs to 4b and is not needed here.

**The staleness background job.** `crates/fubbik/src/main.rs:157` spawns
`staleness::service::spawn_background_scan(pool.clone())`. This slice's job copies its shape
exactly.

## What was measured, not assumed

The scope below is caller-driven. Every claim here came from grepping `apps/web/src`,
`apps/cli/src`, `apps/vscode/src` and `packages/mcp/src` for actual callers, not from reading
route definitions.

**Only one of the seven candidate domains has a caller.** `GET /api/graph` has four web call
sites. `/graph/communities`, `/graph/bridges`, `/graph/redundancy`, `/graph/at`,
`/graph/events`, and every route in `concepts`, `code-index`, `diagram` and `usage` have **zero
callers in any client.** Phase 6 deletes the TypeScript backend, so unported means gone.

**Three of the payload's collections are dead end-to-end, in Node, today.** The chain:

- `usage_event` is written in exactly one place — `packages/api/src/events/handlers.ts:17`, on
  `EVENTS.CHUNK_VIEWED`.
- `CHUNK_VIEWED` is **never emitted.** The only `events.emit` calls in `packages/api/src` are
  `CHUNK_CREATED` and `CHUNK_UPDATED`, both in `chunks/chunk-mutations.ts:119,221`.
- So `usage_event` never receives a row, so `getCoReferenceCounts` always returns zero pairs, so
  `aggregateCoReferences` never writes a `co_referenced` edge, so `detectEmergentConcepts`
  (which reads the same counts) can never create a `concept` vertex.

`coRefEdges` and `concepts` are therefore permanently empty on the Node stack as it runs right
now. This is worth stating plainly because the alternative reading — that Rust would be
*dropping* a working feature — is wrong.

This also settles a claim in `crates/fubbik-db/migrations/0004_behavior_matrix_completion.sql:19`,
which omits `graph_event` and `usage_event` on the grounds that they "have no reader in
`packages/api/src` at all." That is **right about `usage_event` for the wrong reason** (it has a
reader; it has no *writer*) and **wrong about `graph_event`**, which `graph/timeline-service.ts`
reads for `/graph/at` and `/graph/events`. Since those two routes are out of scope, no migration
is needed — but the comment should not be trusted as written, and 4a corrects it in place.

**Nothing renders code or concept nodes.** `apps/web/src/features/graph/graph-view.tsx:39-46`
registers `codeNode` and `conceptNode` in `NODE_TYPES`, `use-graph-state.ts:91` carries a
`showCodeNodes` flag, and `graph-view.tsx:426` renders a toggle for it — but **no code anywhere
in `apps/web/src` constructs a node with either type.** The renderer was never wired up.
Consequently `use-graph-nodes.ts:66-78` builds `governs` edges whose `target` is a code-vertex id
with no corresponding node, and React Flow drops edges with unresolvable endpoints. `governsEdges`
is consumed but renders nothing.

That is why `code-index` is **not** in this slice despite being the sole producer of
`code_file` / `code_symbol` vertices: ~197 LOC of directory walking and symbol extraction feeding
a renderer that does not exist.

**Exhaustive field consumption.** Across all four `/graph` call sites, the web reads: `chunks`
(18 sites), `connections` (4), `chunkTags` (2), `tagTypes` (2), `chunkCodebases`
(`group-strategies.ts:69`), `behaviorRules` and `governsEdges` (`use-graph-nodes.ts:57-58`). It
never reads `communities`, `bridges`, `codeFiles`, `codeSymbols`, `concepts` or `coRefEdges`.

**Space scoping on the graph page is silently broken.** `use-graph-data.ts:31` sends `spaceId`;
`packages/api/src/graph/routes.ts:18` declares the query as `t.Object({ codebaseId, workspaceId })`.
Elysia strips the unknown field, so `getUserGraph` receives `undefined` for the space and returns
every chunk. Workspace scoping works. This is a leftover from
`2026-06-02-codebase-to-space-rename` and is the same failure mode as the `requirements/stats`
bug from 2c — an unknown field dropped rather than rejected, producing unscoped results that look
healthy.

## Architecture

Repository → service → route, as every ported domain.

### `crates/fubbik-db/src/repo/graph.rs` (new)

Five plain-SQL functions ported from `packages/db/src/repository/graph.ts` (107 LOC). No AGE.

| Function | Behaviour |
| --- | --- |
| `list_chunk_meta(pool, user_id, space_id, workspace_id)` | Selects `id, title, type, summary, created_at`. Scoping is `chunk IN (chunks of the workspace's spaces) OR chunk NOT IN (any space)` — **global chunks are always included**, in both the workspace and the single-space case. |
| `list_connections(pool, user_id)` | Connections where source **or** target is one of the user's chunks. Deliberately not space-scoped: Node is not either, and `search-graph.tsx:87` filters client-side against the chunk-id set. |
| `list_chunk_tags_with_types(pool, user_id)` | `chunk_tag ⋈ tag ⟕ tag_type`, filtered on `tag.user_id`. The `tag_type` join is a LEFT join — untyped tags exist and must survive. |
| ~~`list_tag_types(pool, user_id)`~~ | **Not written.** `fubbik_db::repo::tag_type::list` (`tag_type.rs:74`) already returns exactly the columns Node's `db.select().from(tagType)` does; a second copy would drift. The repo module therefore has four functions, not five. |
| `list_chunk_space_mappings(pool, user_id)` | `chunk_space ⋈ space`, filtered on `space.user_id`. Called only when `workspaceId` is set; Node returns `[]` otherwise and the service preserves that conditional. |

### `crates/fubbik-db/src/age.rs` (extended)

`cypher` (line 32) returns a single scalar column aliased `v`. Both queries this slice needs
return several columns:

```
MATCH (r:behavior_rule) RETURN r.id, r.title, r.layer, r.matrixId, r.category
MATCH (r:behavior_rule)-[g:governs]->(c) RETURN r.id, c.id, g.kind
```

Add:

> **Corrected during planning.** This helper **already exists**: `age.rs:104` defines a private
> `cypher_multi(pool, graph, query, columns)`, already used internally at lines 505 and 727. The
> work is publishing it, not writing it.
>
> Conversely, this section missed that `ensure_vertex` (`age.rs:277`), `create_edge` (`age.rs:302`)
> and `delete_edge` (`age.rs:329`) are each hardcoded to the `chunk` label and the `connects` edge
> type, so the behavior-rule primitives are genuinely new code. Net effort is about the same; the
> shape is different. The plan reflects the corrected version.

- `cypher_columns(pool, query, columns) -> Result<Vec<HashMap<String, Value>>, sqlx::Error>` — a
  public wrapper over the existing private `cypher_multi`, fixed to the `"knowledge"` graph exactly
  as `cypher` is. It builds the `AS (col agtype, …)` record definition AGE requires, casts each
  column `::varchar` (sqlx has no `agtype` decoder, so Postgres must stringify before the wire),
  and reuses `parse_agtype` per cell. Nothing about `cypher` or `tests/age.rs` changes.
- `upsert_behavior_rule`, `delete_governs_edges`, `link_governs`, `list_behavior_rule_vertices`,
  `list_governs_edges` — the `behavior_rule` / `governs` primitives, since the existing vertex and
  edge helpers cannot express a label other than `chunk`.

**A Node bug not to copy.** `packages/api/src/graph/service.ts:86-127` unquotes agtype values
with `String(r.id).replace(/"/g, "")` — a raw regex over the stringified value. Any title
containing a double quote is silently corrupted. `parse_agtype` already parses properly; Rust
uses it and inherits nothing.

### `crates/fubbik-api/src/graph/` (new)

`mod.rs`, `dto.rs`, `service.rs`, `routes.rs`. One route: `GET /api/graph`, query
`{ spaceId?, workspaceId? }`.

The service runs the five SQL reads concurrently, then the two AGE reads. Both AGE reads degrade
to an empty vec on any failure — matching Node's `Effect.catchAll` — so an environment without
the extension installed still serves a working graph.

Response DTO, seven fields:

```
chunks, connections, chunkTags, tagTypes, chunkCodebases, behaviorRules, governsEdges
```

`chunkCodebases` keeps its pre-rename wire name because `group-strategies.ts:69` destructures it;
renaming the field is a web change that does not belong in this slice.

The six fields the web never reads are **omitted from the DTO entirely**, not returned as empty
arrays. Because `apps/web` types itself from `openapi.json`, a future reader of a dropped field is
a compile error rather than a silent `undefined` — the absence documents itself.

### `crates/fubbik-api/src/graph/sync.rs` (new)

`spawn_behavior_sync(pool)`, called from `crates/fubbik/src/main.rs` beside the existing
staleness spawn. Mirrors `staleness::service::spawn_background_scan`: a 40-second initial delay
(Node's offset from `startup.ts:78`), then a loop on a configured interval.

The sync body ports `packages/api/src/matrices/graph-sync.ts`: for each matrix, for each rule,
`ensure_vertex("behavior_rule", id)` then `SET` its title/layer/matrixId/category, then
`delete_governs_edges(rule_id)` before re-creating one `governs` edge per `behavior_cell_code`
link. Deleting before rebuilding is what makes a re-run idempotent.

Note that `governs` edges `MATCH` `code_file` / `code_symbol` vertices without creating them
(`graph-sync.ts:57`). With `code-index` out of scope, no such vertices exist, so no `governs`
edges are produced. The code is still ported — it costs nothing and becomes correct the moment a
code index exists — but nobody should expect `governsEdges` to be non-empty.

### Two deliberate divergences from Node

Both were weighed and chosen over strict parity:

1. **Its own interval variable.** Node gates *all three* startup jobs behind
   `STALENESS_SCAN_INTERVAL_HOURS` (`startup.ts:60`), including behavior sync, which has nothing
   to do with staleness. Rust reads `BEHAVIOR_GRAPH_SYNC_INTERVAL_HOURS`, default `24`, `0`
   disables — the same contract as the staleness variable, applied to the right job.
2. **All users, not one.** Node calls `syncBehaviorsToGraph(IMPLICIT_DEV_USER_ID)`
   (`startup.ts:52`), so it syncs exactly one hardcoded user's matrices. Rust iterates every user
   with at least one matrix. On a single-user local install this is identical; on any other it is
   the difference between working and not.

## Scope

- `crates/fubbik-db/src/repo/graph.rs` — five SQL reads.
- `crates/fubbik-db/src/age.rs` — publish `cypher_columns`; add the five `behavior_rule` /
  `governs` primitives.
- `crates/fubbik-api/src/graph/` — DTO, service, `GET /api/graph`, OpenAPI registration.
- `crates/fubbik-api/src/graph/sync.rs` + the `main.rs` spawn.
- `packages/api/src/graph/routes.ts` — `codebaseId` → `spaceId` on all six routes, so the two
  backends agree on scoping while both run.
- Four web call sites to `api`, casts removed: `use-graph-data.ts:27`, `saved-graph-view.tsx:151`,
  `search-graph.tsx:75`, `browse.tsx:24`. Regenerate `api-types.ts`.
- `crates/fubbik-db/migrations/0004_…sql:19` — correct the comment about `graph_event` /
  `usage_event`.
- `apps/web/src/utils/api.ts` — correct the migration doc block, which lists six already-ported
  domains as unported and describes a five-field `UpdateChunkBody` that grew to seventeen in
  `e2b2129`.

### Out of scope

`/graph/communities`, `/graph/bridges`, `/graph/redundancy`, `/graph/at`, `/graph/events`; the
`concepts`, `code-index`, `diagram`, `usage`, `events` and `validation` domains;
`aggregateCoReferences` and `detectEmergentConcepts`; any `graph_event` or `usage_event`
migration; wiring up the `codeNode` / `conceptNode` renderers in the web app. Each is dropped for
a reason recorded above, not by omission.

## Testing

- **Repo tests**, real database, one per function, with scoping stated explicitly: a chunk in the
  target space, a chunk in a *different* space, a chunk in **no** space (must appear), and the
  workspace variant. The "global chunks always included" rule is the one most likely to be lost in
  translation from Drizzle's `NOT IN` subquery.
- **`cypher_columns` tests** following `tests/age.rs`'s existing shape: a multi-column parse, a
  value containing a double quote (the bug Node has — assert Rust returns it intact), and
  degradation against a nonexistent graph.
- **HTTP test for `GET /api/graph`**: response shape; `spaceId` actually filtering; 401
  unauthenticated. The scoping assertion must be shown to **fail before the fix** — otherwise it
  proves nothing, which is exactly how the bug survived the rename.
- **Behavior sync**: run twice against the same data, assert no duplicate edges; assert a rule
  whose title contains a quote round-trips.
- Exit gates as every prior slice: `cargo test --workspace`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo fmt --check`, `SQLX_OFFLINE=true cargo check --workspace
  --all-targets`, and `pnpm exec tsgo -p tsconfig.json --noEmit` clean in `apps/web`.

## Risks

| Risk | Mitigation |
| --- | --- |
| Publishing `cypher_columns` disturbs the 830 lines that depend on `cypher` | It does not: `cypher_multi` already exists and `cypher` is untouched. `tests/age.rs` runs unmodified as the regression check |
| AGE is absent from CI (`pgvector/pgvector:pg18`), so every AGE test passes vacuously there | The plan's Task 0 verifies AGE locally and fails if missing; every report must state whether AGE was live |
| Drizzle's `NOT IN` subquery translated wrong, silently dropping global chunks | The "chunk in no space" case is a named test, not an implied one |
| Behavior sync divergence (all users, own interval) surprises later comparison | Recorded here and in the code; the differential harness will show a difference and it will be expected |
| AGE not installed in some environment | Both AGE reads degrade to empty, matching Node; covered by the nonexistent-graph test |

## What this slice does NOT deliver

The graph page will look exactly as it does today, because everything it renders — chunks,
connections, tags, tag types, island grouping — comes from plain SQL, and the only AGE-derived
thing it renders is the behavior-rule overlay. The visible wins are narrow and worth naming
honestly: **space scoping starts working**, and quoted titles stop being corrupted. The rest of
the value is structural — four call sites type-checked against Rust's own schema, and the first
real consumer of the AGE layer.
