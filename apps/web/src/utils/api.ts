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
// As of this writing, Rust's `openapi.json` has no entry at all for 17
// domains:
//
//   ai, chunk-types, connection-relations, context, density, documents,
//   features, graph, health, matrices, proposals, requirements,
//   saved-graphs, templates, timeline, use-cases, vocabulary
//
// Every call site under those top-level segments (`api.api.<domain>...`)
// is on `legacyApi` instead. This list SHRINKS as Rust ports each domain —
// when one lands, move its call sites back to `api` and drop it from the
// list above (both here and in the call sites themselves).
//
// A few call sites hit routes *inside* an otherwise-ported domain that
// Rust hasn't finished:
//   - chunks: `search/semantic`, `search/federated`, `grouped`,
//     `check-similar`, `clusters`, `import-docs`, `bulk-update`,
//     `{id}/neighbors`, `{id}/suggestions`, `{id}/enrich` have no Rust
//     route yet (see `openapi.json` — only list/create/detail/patch/
//     delete/applies-to/dismiss-staleness/file-refs/history/scan-impact/
//     stale/stale-count/stale-scan-age/suppress-duplicate exist).
//   - chunks: `GET /api/chunks/{id}` exists in Rust, but returns the bare
//     `Chunk` row only — Node's enriched detail shape (`{ chunk,
//     connections, appliesTo, fileReferences, ... }`) isn't there yet.
//     Call sites that need that enrichment (the chunk detail page, the
//     edit page, the graph side panel) stay on `legacyApi` for this one
//     route until Rust's chunk detail response catches up; call sites that
//     only need the bare chunk fields (id/title/type/content/...) stay on
//     `api`, since Rust already serves those correctly.
// These are routed to `legacyApi` for the same reason as the 17 domains —
// the route doesn't exist (or isn't complete) on Rust yet — even though
// `chunks` as a whole is ported.
//
// `VITE_API_URL` (a dedicated URL for the Rust API) doesn't exist yet —
// it's added in a later task. Until then both clients point at
// `VITE_SERVER_URL`; `api`'s requests are routed to Rust at the reverse
// proxy / dev-server level. Once `VITE_API_URL` exists, swap `api`'s base
// below to it.
export const api = createClient(env.VITE_SERVER_URL);

export const legacyApi = treaty<Api>(env.VITE_SERVER_URL, {
    fetch: { credentials: "include" }
});
