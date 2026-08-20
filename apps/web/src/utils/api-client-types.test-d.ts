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

// 6. body shape. This assertion used to run the other way: `CreateChunkBody`
// had no `tags` field, and because Rust has no `deny_unknown_fields`, serde
// dropped it and returned 200 — a silent data-loss bug that this file
// documented with a `@ts-expect-error` rather than fixed.
//
// The body now carries Node's full field set, so `tags` type-checks. Keeping
// this as a POSITIVE assertion is the useful half: if the field is ever
// dropped from the DTO again, this line stops compiling.
api.api.chunks.post({ title: "x", content: "y", type: "note", tags: ["a"] });

// The negative control that makes the line above meaningful — a genuinely
// unknown field must still be rejected, or the body type has collapsed to
// something that accepts anything and proves nothing.
// @ts-expect-error — `notAField` is not part of CreateChunkBody.
api.api.chunks.post({ title: "x", notAField: true });

void _a;
void _b;
void _c;
void _d;
void _e;
