import type { paths } from "./api-types";
import type { Client, EdenLikeResponse, Query } from "./client-types";
import { ApiError, ApiResponseParseError, asNetworkError } from "./errors";

type Method = "get" | "post" | "patch" | "put" | "delete";
const METHODS: readonly string[] = ["get", "post", "patch", "put", "delete"];

export interface ClientOptions {
    credentials?: "include" | "omit" | "same-origin";
    fetch?: (input: string | URL | Request, init?: RequestInit) => Promise<Response>;
    timeoutMs?: number;
}

export type { EdenLikeResponse };

type PathSegments<P extends string> = P extends `/${infer Head}/${infer Rest}`
    ? [Head, ...PathSegments<`/${Rest}`>]
    : P extends `/${infer Last}`
      ? [Last]
      : [];

function buildUrl(base: string, segments: string[], query?: Query): string {
    const path = segments.join("/");
    const prefix = base ? `${base.replace(/\/$/, "")}/${path}` : `/${path}`;
    if (!query) return prefix;

    const search = new URLSearchParams();
    for (const [key, value] of Object.entries(query)) {
        if (value !== undefined && value !== null) search.set(key, String(value));
    }
    const encoded = search.toString();
    return encoded ? `${prefix}?${encoded}` : prefix;
}

async function request(
    base: string,
    segments: string[],
    method: Method,
    body: unknown,
    requestOptions: { query?: Query } | undefined,
    clientOptions: ClientOptions
): Promise<EdenLikeResponse<unknown>> {
    const query = method === "get" ? (body as { query?: Query } | undefined)?.query : requestOptions?.query;
    const payload = method === "get" ? undefined : body;
    const fetcher = clientOptions.fetch ?? globalThis.fetch;

    let response: Response;
    try {
        response = await fetcher(buildUrl(base, segments, query), {
            method: method.toUpperCase(),
            credentials: clientOptions.credentials ?? "include",
            signal: AbortSignal.timeout(clientOptions.timeoutMs ?? 30_000),
            ...(payload === undefined ? {} : { headers: { "content-type": "application/json" }, body: JSON.stringify(payload) })
        });
    } catch (error) {
        throw asNetworkError(error);
    }

    let text: string;
    try {
        text = await response.text();
    } catch (error) {
        throw asNetworkError(error);
    }

    let value: unknown = text;
    if (!text) value = null;
    else if (response.headers.get("content-type")?.includes("application/json")) {
        try {
            value = JSON.parse(text);
        } catch (error) {
            throw new ApiResponseParseError(response.status, { cause: error });
        }
    }

    return response.ok ? { data: value, error: null } : { data: null, error: { status: response.status, value } };
}

export function createClient(base: string, options: ClientOptions = {}): Client {
    const make = (segments: string[]): unknown =>
        new Proxy(function () {} as unknown as object, {
            get(_target, property: string) {
                if (METHODS.includes(property)) {
                    return (body?: unknown, requestOptions?: { query?: Query }) =>
                        request(base, segments, property as Method, body, requestOptions, options);
                }
                return make([...segments, property]);
            },
            apply(_target, _this, args: [Record<string, string>]) {
                const values = Object.values(args[0] ?? {}).map(value => encodeURIComponent(value));
                return make([...segments, ...values]);
            }
        });

    return make([]) as Client;
}

export function unwrapResponse<T>(response: EdenLikeResponse<T>): T {
    if (response.error) throw new ApiError(response.error.status, response.error.value);
    return response.data as T;
}

export type ApiPaths = paths;
export type ApiPathSegments<P extends keyof paths & string> = PathSegments<P>;
