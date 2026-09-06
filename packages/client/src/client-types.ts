import type { paths } from "./api-types";

type Method = "get" | "post" | "patch" | "put" | "delete";

export interface EdenLikeResponse<T> {
    data: T | null;
    error: { status: number; value: unknown } | null;
}

type ContentValue<C> = C extends object ? C[keyof C] : never;

type SuccessBody<Op> = Op extends { responses: infer R }
    ? {
          [K in (200 | 201) & keyof R]: R[K] extends { content: infer C } ? ContentValue<C> : never;
      }[(200 | 201) & keyof R]
    : unknown;

type RequestBodyOf<Op> = Op extends { requestBody?: infer RB }
    ? [RB] extends [{ content: infer C }]
        ? ContentValue<C>
        : undefined
    : undefined;

type QueryValue = string | number | boolean | null | undefined;
export type Query = Record<string, QueryValue>;

type MethodFn<Op> = [RequestBodyOf<Op>] extends [undefined]
    ? (body?: { query?: Query }, options?: { query?: Query }) => Promise<EdenLikeResponse<SuccessBody<Op>>>
    : (body: RequestBodyOf<Op>, options?: { query?: Query }) => Promise<EdenLikeResponse<SuccessBody<Op>>>;

type ChildRoutes<Prefix extends string> = Extract<keyof paths, `${Prefix}/${string}`>;
type NextSegment<Prefix extends string, Route extends string> = Route extends `${Prefix}/${infer Rest}`
    ? Rest extends `${infer Segment}/${string}`
        ? Segment
        : Rest
    : never;
type LiteralSegments<Prefix extends string> = Exclude<NextSegment<Prefix, ChildRoutes<Prefix>>, `{${string}}`>;
type ParamSegment<Prefix extends string> = Extract<NextSegment<Prefix, ChildRoutes<Prefix>>, `{${string}}`>;

type Methods<Route extends string> = {
    [M in Method as Route extends keyof paths
        ? M extends keyof paths[Route]
            ? [paths[Route][M]] extends [undefined]
                ? never
                : M
            : never
        : never]: Route extends keyof paths ? MethodFn<paths[Route][M]> : never;
};

export type BuildNode<Prefix extends string> = Methods<Prefix> & {
    [Segment in LiteralSegments<Prefix>]: BuildNode<`${Prefix}/${Segment}`>;
} & ([ParamSegment<Prefix>] extends [never] ? unknown : (params: Record<string, string>) => BuildNode<`${Prefix}/${ParamSegment<Prefix>}`>);

export type Client = BuildNode<"">;
