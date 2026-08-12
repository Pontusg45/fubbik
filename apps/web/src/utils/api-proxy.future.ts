// PARKED — NOT WIRED IN. Do not import this from app code.
//
// This is a JS Proxy-based API client built over openapi-typescript-generated
// types (see `./api-types.ts`), designed as a drop-in replacement for the
// Eden treaty client (`./api.ts`) once the Rust backend is ready. It
// preserves Eden's call shape (`api.api.chunks.get()`, path params via a
// call segment, etc.) so existing call sites would not need to change.
//
// Verified working at runtime: all 7 tests in `./api-proxy.future.test.ts`
// pass, confirming the Proxy correctly builds URLs, interpolates path
// params, serialises query strings, and mirrors Eden's `{ data, error }`
// response shape.
//
// Why it is NOT wired in (originally attempted in a51c380, then reverted):
//
//   1. [RESOLVED] This repo sets `noUncheckedIndexedAccess: true` in
//      `packages/config/tsconfig.base.json`. An index-signature-based
//      `Client` type made every property access `| undefined`. Fixed by
//      deriving `Client` from the generated `paths` via literal
//      template-string matching (`./api-client-types.ts`) instead of an
//      index signature — see `BuildNode` there.
//   2. Still open: the web app calls ~27 API domains. The Rust backend's
//      `openapi.json` currently covers 14 (activity, chunks, collections,
//      connections, favorites, notifications, plans, search, settings,
//      spaces, stats, tag-types, tags, workspaces) — the rest have no types
//      regardless of how the client is shaped.
//
// What has to be true before this can be adopted:
//   - The Rust backend implements the remaining API domains the web app
//     uses (or the web app is migrated domain-by-domain against a hybrid
//     setup).
//
// When ready: rename this file back to `api.ts` (replacing the Eden
// client), rename `api-proxy.future.test.ts` back to `api.test.ts`, and
// update call sites as needed.

import { env } from "@fubbik/env/web";

import type { Client } from "./api-client-types";
import type { paths } from "./api-types";

type Method = "get" | "post" | "patch" | "put" | "delete";

const METHODS: readonly string[] = ["get", "post", "patch", "put", "delete"];

export interface EdenLikeResponse<T> {
    data: T | null;
    error: { status: number; value: unknown } | null;
}

/**
 * Recursively maps the generated OpenAPI `paths` object into Eden's
 * property-access call shape, so existing call sites keep compiling.
 *
 * `api.api.chunks({ id }).get()` maps to GET /api/chunks/{id}.
 */
type PathSegments<P extends string> = P extends `/${infer Head}/${infer Rest}`
    ? [Head, ...PathSegments<`/${Rest}`>]
    : P extends `/${infer Last}`
      ? [Last]
      : [];

// `Client` is derived from the generated OpenAPI `paths` by literal
// template-string matching (see `./api-client-types.ts`), producing literal
// keys instead of an index signature — so `noUncheckedIndexedAccess` never
// applies here; an unknown segment is a hard "does not exist" error instead
// of "possibly undefined" everywhere.

function buildUrl(base: string, segments: string[], query?: Record<string, string | undefined>): string {
    const path = segments.join("/");
    const url = `${base}/${path}`;
    if (!query) return url;

    const search = new URLSearchParams();
    for (const [k, v] of Object.entries(query)) {
        if (v !== undefined) search.set(k, v);
    }
    const qs = search.toString();
    return qs ? `${url}?${qs}` : url;
}

async function request(
    base: string,
    segments: string[],
    method: Method,
    body?: unknown,
    options?: { query?: Record<string, string | undefined> }
): Promise<EdenLikeResponse<unknown>> {
    // GET takes its query from the first argument; other verbs take a body.
    const query = method === "get" ? (body as { query?: Record<string, string> })?.query : options?.query;
    const payload = method === "get" ? undefined : body;

    const res = await fetch(buildUrl(base, segments, query), {
        method: method.toUpperCase(),
        credentials: "include",
        ...(payload === undefined
            ? {}
            : { headers: { "content-type": "application/json" }, body: JSON.stringify(payload) })
    });

    const value = res.headers.get("content-type")?.includes("application/json")
        ? await res.json()
        : await res.text();

    return res.ok
        ? { data: value, error: null }
        : { data: null, error: { status: res.status, value } };
}

export function createClient(base: string): Client {
    const make = (segments: string[]): unknown =>
        new Proxy(function () {} as unknown as object, {
            get(_target, prop: string) {
                if (METHODS.includes(prop)) {
                    return (body?: unknown, options?: { query?: Record<string, string> }) =>
                        request(base, segments, prop as Method, body, options);
                }
                return make([...segments, prop]);
            },
            // A call segment supplies path params: chunks({ id: "abc" })
            apply(_target, _this, args: [Record<string, string>]) {
                const values = Object.values(args[0] ?? {});
                return make([...segments, ...values]);
            }
        });

    return make([]) as Client;
}

export const api = createClient(env.VITE_SERVER_URL);

// Referenced so the generated types participate in type-checking even
// though the Proxy is dynamically typed at the boundary.
export type ApiPaths = paths;
export type ApiPathSegments<P extends keyof paths & string> = PathSegments<P>;
