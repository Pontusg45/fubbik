// OpenAPI-typed Proxy client over `./api-types.ts` (generated from Rust's
// `openapi.json`). Preserves Eden's call shape (`api.api.chunks.get()`, path
// params via a call segment). Routes not in the spec must use `legacyApi`
// — see `./api.ts` and `./legacy-api-routes.ts`.

import { ApiResponseParseError, asNetworkError } from "@/lib/api-errors";

import type { Client, EdenLikeResponse } from "./api-client-types";
import type { paths } from "./api-types";

type Method = "get" | "post" | "patch" | "put" | "delete";

const METHODS: readonly string[] = ["get", "post", "patch", "put", "delete"];
const REQUEST_TIMEOUT_MS = 30_000;

export type { EdenLikeResponse };

type PathSegments<P extends string> = P extends `/${infer Head}/${infer Rest}`
    ? [Head, ...PathSegments<`/${Rest}`>]
    : P extends `/${infer Last}`
      ? [Last]
      : [];

function buildUrl(base: string, segments: string[], query?: Record<string, string | undefined>): string {
    const path = segments.join("/");
    const prefix = base ? `${base.replace(/\/$/, "")}/${path}` : `/${path}`;
    if (!query) return prefix;

    const search = new URLSearchParams();
    for (const [k, v] of Object.entries(query)) {
        if (v !== undefined) search.set(k, v);
    }
    const qs = search.toString();
    return qs ? `${prefix}?${qs}` : prefix;
}

async function request(
    base: string,
    segments: string[],
    method: Method,
    body?: unknown,
    options?: { query?: Record<string, string | undefined> }
): Promise<EdenLikeResponse<unknown>> {
    const query = method === "get" ? (body as { query?: Record<string, string> })?.query : options?.query;
    const payload = method === "get" ? undefined : body;

    let res: Response;
    try {
        res = await fetch(buildUrl(base, segments, query), {
            method: method.toUpperCase(),
            credentials: "include",
            signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
            ...(payload === undefined ? {} : { headers: { "content-type": "application/json" }, body: JSON.stringify(payload) })
        });
    } catch (error) {
        throw asNetworkError(error);
    }

    let text: string;
    try {
        text = await res.text();
    } catch (error) {
        throw asNetworkError(error);
    }
    let value: unknown = text;
    if (!text) {
        value = null;
    } else if (res.headers.get("content-type")?.includes("application/json")) {
        try {
            value = JSON.parse(text);
        } catch (error) {
            throw new ApiResponseParseError(res.status, { cause: error });
        }
    }

    return res.ok ? { data: value, error: null } : { data: null, error: { status: res.status, value } };
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
            apply(_target, _this, args: [Record<string, string>]) {
                const values = Object.values(args[0] ?? {}).map(value => encodeURIComponent(value));
                return make([...segments, ...values]);
            }
        });

    return make([]) as Client;
}

export type ApiPaths = paths;
export type ApiPathSegments<P extends keyof paths & string> = PathSegments<P>;
