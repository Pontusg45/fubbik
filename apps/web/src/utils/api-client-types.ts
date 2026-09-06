// Derives the Eden-like client's property-access type directly from the
// generated OpenAPI `paths` (see `./api-types.ts`), by literal template-
// string matching over `keyof paths` — no index signature involved.
//
// Under `noUncheckedIndexedAccess: true` (`packages/config/tsconfig.base.json`),
// an index-signature-based `Client` type (`{ [segment: string]: ... }`) makes
// every property access `| undefined`, regardless of how many real routes
// exist. Producing literal keys here means `noUncheckedIndexedAccess` never
// applies to this type at all — access to an unknown segment is a hard
// "does not exist" error instead of "possibly undefined".

import type { paths } from "./api-types";

type Method = "get" | "post" | "patch" | "put" | "delete";

/**
 * Eden treaty's response envelope, reproduced here so the derived `Client`
 * type matches what call sites already destructure (`const { data, error } =
 * await api.api.chunks.get()`). The runtime shape comes from `request()` in
 * `./openapi-client.ts` — this is only the type.
 */
export interface EdenLikeResponse<T> {
    data: T | null;
    error: { status: number; value: unknown } | null;
}

/** The value type of an OpenAPI `content` object, regardless of media type key
 * (`"application/json"`, `"text/plain"`, ...) — content objects always have
 * exactly one key in this API's generated types. */
type ContentValue<C> = C extends object ? C[keyof C] : never;

/** The 2xx response body for an `operations[...]` entry. Every route in this
 * API succeeds with either 200 or 201 (never both) — see `./api-types.ts`. */
type SuccessBody<Op> = Op extends { responses: infer R }
    ? {
          [K in (200 | 201) & keyof R]: R[K] extends { content: infer C } ? ContentValue<C> : never;
      }[(200 | 201) & keyof R]
    : unknown;

/** The JSON request body type for an `operations[...]` entry, or `undefined`
 * when the route has no body at all — openapi-typescript emits `requestBody?:
 * never` for those (mostly GETs), never `requestBody?: { content: ... }`, so
 * "has a body" and "body is required" coincide here (verified against every
 * emission in `./api-types.ts`).
 *
 * The `[RB] extends [...]` tuple wrapping isn't optional: `RB` is a naked
 * type parameter here, and a bare `RB extends {content: infer C} ? ... :
 * undefined` distributes when `RB` is instantiated to `never` (the type
 * openapi-typescript gives the property) — a distributive conditional over
 * `never` collapses to `never` itself, not to the `undefined` branch,
 * silently reintroducing "no body is checked" for every no-body route. Same
 * pitfall as `Methods` above; confirmed empirically before writing this. */
type RequestBodyOf<Op> = Op extends { requestBody?: infer RB }
    ? [RB] extends [{ content: infer C }]
        ? ContentValue<C>
        : undefined
    : undefined;

/** The method's call signature: a body param is required and typed from the
 * route's `requestBody` when one exists, omittable when it doesn't. This is
 * what makes an extra or missing field on a real POST/PATCH/PUT body a
 * compile error instead of a silent 2xx that drops data (the nine bugs this
 * phase fixed — see the `CreateChunkBody` negative control in
 * `api-client-types.test-d.ts`). */
type MethodFn<Op> = [RequestBodyOf<Op>] extends [undefined]
    ? // No request body on this route (mostly GETs). The runtime
      // (`request()` in `./openapi-client.ts`) reads query params for GET
      // off the *first* argument (`body.query`), not the second — every
      // call site in this codebase calls GETs as `.get({ query: {...} })`,
      // never with a second `options` argument. So "no body" still needs to
      // accept the query-carrier shape here, or every existing GET call
      // site would fail to compile despite passing no real body.
      (
          body?: { query?: Record<string, unknown> },
          options?: { query?: Record<string, unknown> }
      ) => Promise<EdenLikeResponse<SuccessBody<Op>>>
    : (body: RequestBodyOf<Op>, options?: { query?: Record<string, unknown> }) => Promise<EdenLikeResponse<SuccessBody<Op>>>;

/** Path keys that continue below `Prefix`, e.g. "/api/chunks" under "/api". */
type ChildRoutes<Prefix extends string> = Extract<keyof paths, `${Prefix}/${string}`>;

/** The single next segment after `Prefix` in `Route`. */
type NextSegment<Prefix extends string, Route extends string> = Route extends `${Prefix}/${infer Rest}`
    ? Rest extends `${infer Seg}/${string}`
        ? Seg
        : Rest
    : never;

type LiteralSegments<Prefix extends string> = Exclude<NextSegment<Prefix, ChildRoutes<Prefix>>, `{${string}}`>;

type ParamSegment<Prefix extends string> = Extract<NextSegment<Prefix, ChildRoutes<Prefix>>, `{${string}}`>;

// openapi-typescript emits every method key on every path entry — undefined
// methods are stubbed as `put?: never` rather than omitted, so checking key
// membership alone (`M extends keyof paths[Route]`) admits every method on
// every route. An optional `never`-valued property reads as `undefined`
// through an indexed access (`never | undefined` collapses to `undefined`),
// not as `never` itself — so the exclusion has to test against `undefined`,
// wrapped in a tuple (`[X] extends [undefined]`) so the check doesn't
// distribute over `X` when it's a naked type parameter.
type Methods<Route extends string> = {
    [M in Method as Route extends keyof paths
        ? M extends keyof paths[Route]
            ? [paths[Route][M]] extends [undefined]
                ? never
                : M
            : never
        : never]: Route extends keyof paths ? MethodFn<paths[Route][M]> : never;
};

export type BuildNode<Prefix extends string> = Methods<Prefix> & { [Seg in LiteralSegments<Prefix>]: BuildNode<`${Prefix}/${Seg}`> } & ([
        ParamSegment<Prefix>
    ] extends [never]
        ? unknown
        : (params: Record<string, string>) => BuildNode<`${Prefix}/${ParamSegment<Prefix>}`>);

export type Client = BuildNode<"">;
