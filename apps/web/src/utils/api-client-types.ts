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
        : never]: (body?: unknown, options?: { query?: Record<string, unknown> }) => Promise<unknown>;
};

export type BuildNode<Prefix extends string> = Methods<Prefix> &
    { [Seg in LiteralSegments<Prefix>]: BuildNode<`${Prefix}/${Seg}`> } &
    ([ParamSegment<Prefix>] extends [never]
        ? unknown
        : (params: Record<string, string>) => BuildNode<`${Prefix}/${ParamSegment<Prefix>}`>);

export type Client = BuildNode<"/api">;
