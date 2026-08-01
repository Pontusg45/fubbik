import { env } from "@fubbik/env/web";

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

// Declared as an `interface` (not a `type` alias) so the self-reference
// resolves consistently across module boundaries — with a recursive type
// alias, tsgo's cross-file inference collapses the intersection and loses
// the index signature branch, turning every property access into a hard
// "does not exist" error instead of the expected `noUncheckedIndexedAccess`
// possibly-undefined warning.
interface Client {
    [segment: string]: Client & ((params: Record<string, string>) => Client) & {
        [M in Method]: (
            body?: unknown,
            options?: { query?: Record<string, string | undefined> }
        ) => Promise<EdenLikeResponse<unknown>>;
    };
}

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
