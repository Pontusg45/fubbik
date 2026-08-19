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
// As of this writing, Rust's `openapi.json` has no entry at all for 14
// domains:
//
//   ai, chunk-types, comments, connection-relations, context, density,
//   features, file-refs (top-level list/lookup — the chunks/{id}/file-refs
//   sub-resource IS on Rust), graph, health, learning-paths, matrices,
//   requirements, timeline
//
// Every call site under those top-level segments (`api.api.<domain>...`)
// is on `legacyApi` instead. This list SHRINKS as Rust ports each domain —
// when one lands, move its call sites back to `api` and drop it from the
// list above (both here and in the call sites themselves).
//
// `documents` and `vocabulary` were ported off this list once their DTOs
// were checked field-by-field against every call site's request body and
// response shape — all matched, so every call site moved to `api`.
//
// `saved-graphs` moved partially: `DELETE /api/saved-graphs/{id}` moved to
// `api`, but `GET /api/saved-graphs/{id}` and `PATCH .../{id}` (both call
// sites, in `features/graph/saved-graph-view.tsx`) stay on `legacyApi`.
// Rust's `openapi.json` documents `SavedGraph.positions` as
// `Record<string, {start, end}>` instead of `Record<string, {x, y}>` — a
// utoipa schema-name collision: `fubbik_db::repo::saved_graph::Position`
// ({x, y}) and `fubbik_api::vocabulary::parser::Position` ({start, end})
// both register as `#/components/schemas/Position`, so one silently
// overwrites the other in the generated schema. Actual Rust runtime
// behaviour is presumably still {x, y} (the DTOs use the right struct),
// but the generated TS types are wrong until the crate disambiguates the
// two schemas — so any call site that reads or writes `positions` stays
// on `legacyApi` for now.
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
