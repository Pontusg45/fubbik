import { useQuery, type UseQueryOptions } from "@tanstack/react-query";

import { ApiError } from "@/lib/api-errors";
import { unwrapEden } from "@/utils/eden";

type QueryKey = readonly unknown[];
const EMPTY_API_LIST: never[] = [];

/**
 * Shape of what eden treaty returns — but intentionally loose. Declaring it
 * as `Promise<{ data: TData; error: unknown }>` here would force callers to
 * cast (`as unknown as Promise<…>`) because eden's response type carries
 * extra brands. We instead accept any promise-returning thunk and trust
 * `unwrapEden` (which reads `.data` / `.error` at runtime) to handle it.
 */
type EdenThunk = () => Promise<{ data: unknown; error: unknown }>;

export interface UseApiQueryOptions<TData> extends Omit<UseQueryOptions<TData, Error, TData, QueryKey>, "queryFn" | "queryKey"> {
    /**
     * Eden treaty call. Pass the bare `api.api.X.get()` thunk; the response
     * is auto-unwrapped via `unwrapEden` so callers never touch the
     * `{ data, error }` envelope.
     */
    queryFn: EdenThunk;
    /**
     * Fallback returned only when the HTTP status is explicitly listed in
     * `fallbackStatuses`. Transport, 5xx, and response-contract failures are
     * always surfaced through React Query's error state.
     */
    fallback?: TData;
    /** HTTP statuses for which `fallback` represents an expected absence. */
    fallbackStatuses?: readonly number[];
}

/**
 * Thin wrapper over `useQuery`. Mirrors `useApiMutation` for the read path —
 * consolidates the `unwrapEden(await api.api.X.get())` plus
 * `try { … } catch { return [] }` pattern that recurs ~130 times across the
 * web app, almost always with a stale time of 60_000 ms.
 *
 * Sane defaults:
 *   - staleTime 60_000 ms (can be overridden)
 *   - expected status → fallback only when explicitly configured
 *
 * Example:
 *   const q = useApiQuery({
 *     queryKey: ["tags"],
 *     queryFn: () => api.api.tags.get(),
 *     fallback: [],
 *     fallbackStatuses: [404],
 *   });
 */
export function useApiQuery<TData>(options: UseApiQueryOptions<TData> & { queryKey: QueryKey }) {
    const { queryFn, fallback, fallbackStatuses = [], staleTime, ...rest } = options;
    return useQuery<TData, Error, TData, QueryKey>({
        ...rest,
        staleTime: staleTime ?? 60_000,
        queryFn: async () => {
            try {
                return unwrapEden(await queryFn()) as TData;
            } catch (err) {
                if (fallback !== undefined && err instanceof ApiError && fallbackStatuses.includes(err.status)) return fallback;
                throw err;
            }
        }
    });
}

/**
 * List-specialized API query whose `data` is always a stable array.
 *
 * React Query leaves `data` undefined until the first result. Writing
 * `query.data ?? []` at each call site creates a new array every render,
 * which invalidates memo/effect dependencies even though the list is still
 * empty. This wrapper shares one empty fallback until real data arrives and
 * keeps it as render-time data while requests are pending. HTTP and response
 * contract failures remain visible through React Query's error state.
 */
export function useApiListQuery<TItem>(options: UseApiQueryOptions<TItem[]> & { queryKey: QueryKey }) {
    const query = useApiQuery<TItem[]>({
        ...options,
        fallback: options.fallback
    });

    return {
        ...query,
        data: query.data ?? (EMPTY_API_LIST as TItem[])
    };
}
