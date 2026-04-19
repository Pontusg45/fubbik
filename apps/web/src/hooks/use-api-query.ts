import { useQuery, type UseQueryOptions } from "@tanstack/react-query";

import { unwrapEden } from "@/utils/eden";

type QueryKey = readonly unknown[];

export interface UseApiQueryOptions<TData>
    extends Omit<UseQueryOptions<TData, Error, TData, QueryKey>, "queryFn" | "queryKey"> {
    /**
     * Eden treaty call returning the raw `{ data, error }` Promise. The
     * result is auto-unwrapped via `unwrapEden` so callers never touch the
     * envelope.
     */
    queryFn: () => Promise<{ data: TData; error: unknown }>;
    /**
     * Fallback value returned if the fetch throws. When set, errors are
     * swallowed — the query resolves with `fallback` instead of going into
     * error state. Useful for list pages where an empty array is the
     * natural "no data" fallback. Omit to let react-query surface errors
     * the normal way.
     */
    fallback?: TData;
}

/**
 * Thin wrapper over `useQuery`. Mirrors `useApiMutation` for the read path —
 * consolidates the `unwrapEden(await api.api.X.get())` plus
 * `try { … } catch { return [] }` pattern that recurs ~130 times across the
 * web app, almost always with a stale time of 60_000 ms.
 *
 * Sane defaults:
 *   - staleTime 60_000 ms (can be overridden)
 *   - swallow errors → fallback when `fallback` is provided
 *
 * Example:
 *   const q = useApiQuery({
 *     queryKey: ["tags"],
 *     queryFn: () => api.api.tags.get(),
 *     fallback: [],
 *   });
 */
export function useApiQuery<TData>(options: UseApiQueryOptions<TData> & { queryKey: QueryKey }) {
    const { queryFn, fallback, staleTime, ...rest } = options;
    return useQuery<TData, Error, TData, QueryKey>({
        ...rest,
        staleTime: staleTime ?? 60_000,
        queryFn: async () => {
            try {
                return unwrapEden(await queryFn()) as TData;
            } catch (err) {
                if (fallback !== undefined) return fallback;
                throw err;
            }
        },
    });
}
