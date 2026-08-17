import type { Client } from "./api-client-types";

// `Client` is built from `BuildNode<"">`, so it already includes the "api"
// segment as its root child — no synthetic wrapper needed.
declare const api: Client;

// 1. plain chain (140 `.get(` sites)
const _a = api.api.spaces.get();
// 2. param call (91 sites)
const _b = api.api.chunks({ id: "x" }).get();
// 3. literal hyphenated bracket (21 sites)
// (chunk-types/matrices aren't in the 14 domains this openapi.json ports
// yet — tag-types exercises the same shape against a route that exists.)
const _c = api.api["tag-types"].get();
// 4. param-call into bracket — same shape as the deepest real chain
// (matrices({id}).cells({cellId})["test-results"].get(), cell-panel.tsx:90),
// substituted for a route that exists in the current 14-domain surface.
const _d = api.api.chunks({ id: "x" })["dismiss-staleness"].post();
// 5. query options (64 sites)
const _e = api.api.chunks.stale.count.get({ query: {} });

// Negative controls — these MUST error, or the type has collapsed to `any`
// @ts-expect-error unknown segment
api.api.chunks.nonexistent.get();
// @ts-expect-error method not defined on this route
api.api.spaces.put();

// 6. body shape — `CreateChunkBody` (crates/fubbik-api/src/chunks/dto.rs) has
// no `tags` field and no `deny_unknown_fields`, so Rust silently drops it and
// returns 200. This is the exact shape of the tags/alternatives/consequences
// data-loss bug from this phase; the negative control below is the check
// that would have caught it at authoring time.
// @ts-expect-error — `tags` is not a field of Rust's CreateChunkBody.
api.api.chunks.post({ title: "x", content: "y", type: "note", tags: ["a"] });

void _a;
void _b;
void _c;
void _d;
void _e;
