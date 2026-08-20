import { treaty } from "@elysiajs/eden";
import type { Api } from "@fubbik/api";
import { env } from "@fubbik/env/web";

import { createClient } from "./api-proxy.future";

// ---------------------------------------------------------------------------
// Hybrid API client: Rust for what it serves, Node for what it doesn't yet.
// ---------------------------------------------------------------------------
//
// `api` is typed straight from Rust's own `openapi.json` (see
// `./api-types.ts`, regenerated from the `fubbik-api` crate) via the
// property-access Proxy in `./api-proxy.future.ts`. Use this for any domain
// Rust serves.
//
// `legacyApi` is Eden treaty typed from Node's Elysia `Api` type
// (`packages/api/src/index.ts`). It is TEMPORARY scaffolding for domains
// Rust has not ported yet — Node keeps running (it still owns SSR and
// authentication regardless), so nothing user-facing degrades while the
// port is incomplete.
//
// As of this writing, Rust's `openapi.json` has no entry at all for 10
// domains:
//
//   ai, comments, context, density, file-refs (top-level list/lookup — the
//   chunks/{id}/file-refs sub-resource IS on Rust), graph, health,
//   learning-paths, matrices, timeline
//
// Every call site under those top-level segments (`api.api.<domain>...`)
// is on `legacyApi` instead. This list SHRINKS as Rust ports each domain —
// when one lands, move its call sites back to `api` and drop it from the
// list above (both here and in the call sites themselves).
//
// `requirements` is fully on `api`, including `GET /requirements/coverage`
// and `GET /requirements/traceability` (ported alongside the migration).
//
// Migrating those call sites is what VERIFIES a port: request bodies and
// responses are typed from `openapi.json`, so a wrong shape is a compile
// error. Leaving sites on `legacyApi` after a domain lands throws that away
// — all 27 requirements sites sat here long after the port, and removing
// their `as any` casts surfaced three real bugs (two always-empty reads in
// the web app, and `requirements/stats` silently ignoring `spaceId` because
// Rust's `StatsQuery` was missing `rename_all = "camelCase"`; serde drops
// unknown fields rather than rejecting them, so it returned unscoped totals
// while looking healthy). A ported domain is not a migrated domain — check
// this for any domain marked done.
//
// `features` (13 endpoints incl. the `chunks/{id}/deltas` sub-routes) and
// the `chunk-types`/`connection-relations` catalogs were ported off this
// list and every call site moved to `api`, casts removed.
//
// `documents` and `vocabulary` were ported off this list once their DTOs
// were checked field-by-field against every call site's request body and
// response shape — all matched, so every call site moved to `api`.
//
// `saved-graphs` is fully on `api`. It was partially pinned for a while by a
// utoipa schema-name collision — `fubbik_db::repo::saved_graph::Position`
// ({x, y}) and `fubbik_api::vocabulary::parser::Position` ({start, end}) both
// registered as `#/components/schemas/Position`, so one silently overwrote the
// other and `SavedGraph.positions` was published with the wrong shape. Fixed by
// `#[schema(as = GraphNodePosition)]` / `#[schema(as = TextSpan)]`. That class
// of bug is now caught at the source by `crates/fubbik-api/tests/schema_names.rs`
// — a collision is invisible in the finished spec, so it is checked where the
// two definitions are still distinguishable.
//
// `GET /api/chunks/{id}` now returns Node's **enriched** detail shape
// (`{ chunk, connections, spaces, appliesTo, fileReferences, tags,
// requirements, healthScore, _appliedFeatures, _hasDeltas, allDeltas,
// deltas }`), so every call site that needed it — the chunk detail page,
// the edit page, the graph side panel, compose, the group list, the
// bulk tag editor — moved to `api`.
//
// The two chunk sub-resources moved with it, and fixing them fixed a
// silent data-loss bug in both directions: Rust's `PUT .../applies-to` and
// `PUT .../file-refs` published `{patterns: string[]}` / `{paths:
// string[]}`, while Node (and this app) send a bare array of
// `{pattern, note}` / `{path, anchor, relation}`. Against Rust every such
// request 400'd, and the edit/new pages swallowed it in
// `catch { /* non-critical */ }` — patterns and file refs were never
// saved. The GET side dropped the same three columns (`note`, `anchor`,
// `relation`) from its projection, so even Node-written values were
// invisible through Rust. Both halves are fixed and pinned by tests.
//
// A few call sites still hit routes *inside* an otherwise-ported domain
// that Rust hasn't finished:
//   - chunks: `search/semantic`, `search/federated`, `grouped`,
//     `check-similar`, `clusters`, `import-docs` (+ `import-docs/preview`),
//     `bulk-update`, `{id}/neighbors`, `{id}/suggestions`, `{id}/enrich`,
//     `{id}/archive`, `{id}/restore`, `archived`, `merge`, `bulk` have no
//     Rust route yet.
//   - chunks: `PATCH /api/chunks/{id}` exists on Rust but its
//     `UpdateChunkBody` accepts only title/content/type/rationale/
//     consequences. Call sites that set `tags`, `alternatives`,
//     `reviewStatus`, `isEntryPoint` or `scope` stay on `legacyApi` until
//     it grows those fields — sending them to Rust is not an error, it is
//     a SILENT no-op, because serde drops unknown fields rather than
//     rejecting them. That is the same failure mode that made
//     `requirements/stats` return unscoped totals while looking healthy.
// These are routed to `legacyApi` for the same reason as the remaining
// domains — the route (or the field) doesn't exist on Rust yet — even
// though `chunks` as a whole is ported.
//
// `api` hits the Rust server directly via `VITE_API_URL` (port 3100 by
// default). `legacyApi` stays on `VITE_SERVER_URL` (Node, port 3000), which
// also keeps serving SSR and authentication for the whole app.
export const api = createClient(env.VITE_API_URL);

export const legacyApi = treaty<Api>(env.VITE_SERVER_URL, {
    fetch: { credentials: "include" }
});
