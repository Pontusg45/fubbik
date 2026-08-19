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
// `requirements` was ported off this list once every call site's request
// body and response shape was checked field-by-field against Rust's DTOs
// (see `RequirementDetail`/`Requirement`/etc. doc comments in
// `openapi.json`) — all matched except one query-param casing bug (see the
// `requirements/stats` note below), so every call site moved to `api`
// except two sub-routes Rust doesn't serve at all yet:
//   - `GET /requirements/coverage` (`src/routes/coverage.tsx`)
//   - `GET /requirements/traceability` (`src/features/coverage/
//     traceability-content.tsx`)
// Also found in that audit: `GET /api/requirements/stats`'s query param is
// `space_id` (snake_case) while every sibling route in the domain uses
// `spaceId` (camelCase) — Rust's `StatsQuery`
// (`crates/fubbik-api/src/requirements/dto.rs:148-151`) is missing the
// `#[serde(rename_all = "camelCase")]` its neighbor `ExportAllQuery` has.
// The request client's query type is untyped (`Record<string, unknown>`),
// so this compiles either way — passing `spaceId` would silently be
// ignored by the server. The call site in `src/routes/requirements.tsx`
// sends `space_id` to match Rust's actual (buggy) contract; the mismatch
// itself is a Rust-side fix this migration didn't make.
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
