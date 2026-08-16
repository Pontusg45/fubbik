// LIVE — this is the API client the app uses. Imported by `./api.ts`, which
// exports it as `api`. (The `.future` suffix is historical; renaming the file
// is deliberate churn nobody has spent yet.)
//
// A JS Proxy-based client over openapi-typescript-generated types
// (`./api-types.ts`), preserving Eden's call shape (`api.api.chunks.get()`,
// path params via a call segment) so call sites did not have to change.
// Runtime behaviour is covered by `./api-proxy.future.test.ts`.
//
// It is typed from RUST's `openapi.json`, so it only knows routes Rust
// actually serves. Anything Rust does not serve must go through `legacyApi`
// (Eden -> Node) — see the inventory in `./api.ts`.
//
// History, kept because it explains the design:
//   1. [RESOLVED] `noUncheckedIndexedAccess: true` made every property access
//      on an index-signature `Client` type `| undefined`, which is where the
//      ~1,016 errors came from. Fixed by deriving `Client` from the generated
//      `paths` via literal template-string matching — see `BuildNode` in
//      `./api-client-types.ts`. The count tracked call-site chains, not
//      endpoints, so porting more domains would never have reduced it.
//   2. [RESOLVED, differently than planned] The web app calls more domains
//      than Rust serves. Rather than waiting for parity, the app runs a
//      hybrid: this client for what Rust serves, `legacyApi` for the rest.
//
// WARNING, learned the hard way: an `as any` cast on this client erases the
// types downstream, which is exactly how archive/restore/enrich/bulk-update,
// comments, and proposals silently 404ed after the swap — they were calling
// Rust for Node-only routes and nothing complained. If you reach for
// `as any`, confirm which backend serves the route first.

import { env } from "@fubbik/env/web";

import type { Client, EdenLikeResponse } from "./api-client-types";
import type { paths } from "./api-types";

type Method = "get" | "post" | "patch" | "put" | "delete";

const METHODS: readonly string[] = ["get", "post", "patch", "put", "delete"];

export type { EdenLikeResponse };

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

export const api = createClient(env.VITE_API_URL);

// Referenced so the generated types participate in type-checking even
// though the Proxy is dynamically typed at the boundary.
export type ApiPaths = paths;
export type ApiPathSegments<P extends keyof paths & string> = PathSegments<P>;
