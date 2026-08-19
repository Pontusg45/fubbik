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
// As of this writing, Rust's `openapi.json` has no entry at all for 13
// domains:
//
//   ai, chunk-types, comments, connection-relations, context, density,
//   features, file-refs (top-level list/lookup — the chunks/{id}/file-refs
//   sub-resource IS on Rust), graph, health, learning-paths, matrices,
//   timeline
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
// A few call sites hit routes *inside* an otherwise-ported domain that
// Rust hasn't finished:
//   - chunks: `search/semantic`, `search/federated`, `grouped`,
//     `check-similar`, `clusters`, `import-docs` (+ `import-docs/preview`),
//     `bulk-update`, `{id}/neighbors`, `{id}/suggestions`, `{id}/enrich`,
//     `{id}/deltas/{featureId}` (part of the `features` domain, not
//     `chunks`, despite the URL prefix) have no Rust route yet (see
//     `openapi.json` — only list/create/detail/patch/delete/applies-to/
//     dismiss-staleness/file-refs/history/scan-impact/stale/stale-count/
//     stale-scan-age/suppress-duplicate exist).
//   - chunks: `GET /api/chunks/{id}` exists in Rust, but returns the bare
//     `Chunk` row only — Node's enriched detail shape (`{ chunk,
//     connections, appliesTo, fileReferences, ... }`) isn't there yet.
//     Call sites that need that enrichment (the chunk detail page, the
//     edit page, the graph side panel) stay on `legacyApi` for this one
//     route until Rust's chunk detail response catches up; call sites that
//     only need the bare chunk fields (id/title/type/content/...) stay on
//     `api`, since Rust already serves those correctly.
// These are routed to `legacyApi` for the same reason as the 14 domains —
// the route doesn't exist (or isn't complete) on Rust yet — even though
// `chunks` as a whole is ported.
//
// `api` hits the Rust server directly via `VITE_API_URL` (port 3100 by
// default). `legacyApi` stays on `VITE_SERVER_URL` (Node, port 3000), which
// also keeps serving SSR and authentication for the whole app.
export const api = createClient(env.VITE_API_URL);

export const legacyApi = treaty<Api>(env.VITE_SERVER_URL, {
    fetch: { credentials: "include" }
});
