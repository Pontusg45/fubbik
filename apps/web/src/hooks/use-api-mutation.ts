import { useMutation, useQueryClient, type UseMutationOptions } from "@tanstack/react-query";
import { toast } from "sonner";

import { unwrapEden } from "@/utils/eden";

type QueryKey = readonly unknown[];

export interface UseApiMutationOptions<TData, TVariables>
    extends Omit<UseMutationOptions<TData, Error, TVariables>, "mutationFn" | "onSuccess" | "onError"> {
    /**
     * Eden treaty call. Receives the mutation variables and should return the
     * raw `{ data, error }` Promise. The result is automatically unwrapped via
     * `unwrapEden`, so callers never touch the envelope.
     */
    mutationFn: (vars: TVariables) => Promise<{ data: TData; error: unknown }>;
    /**
     * Query keys to invalidate on success. A single key is fine; an array of
     * keys invalidates all of them. The `|` separator isn't supported —
     * react-query expects discrete keys.
     */
    invalidate?: QueryKey | QueryKey[];
    /**
     * Toast shown on success. Pass `false` to stay silent, a string for a fixed
     * message, or a function to derive it from the mutation result.
     */
    successToast?: string | ((data: TData, vars: TVariables) => string) | false;
    /**
     * Toast shown on error. Pass `false` to suppress, a string for a fixed
     * message, or a function that receives the Error. Default: "Request failed".
     */
    errorToast?: string | ((err: Error, vars: TVariables) => string) | false;
    /** Optional success hook, fired after invalidation + toast. */
    onSuccess?: (data: TData, vars: TVariables) => void;
    /** Optional error hook, fired after the error toast. */
    onError?: (err: Error, vars: TVariables) => void;
}

/**
 * Thin wrapper over `useMutation` that collapses the three pieces of
 * boilerplate that recur almost 100 times across the web app:
 *   1. `unwrapEden(await …)` around every eden call
 *   2. `queryClient.invalidateQueries({ queryKey })` on success
 *   3. `toast.error("…")` on failure
 *
 * The goal is that the common case becomes a one-liner, but the escape
 * hatches (custom success/error hooks, pass-through react-query options)
 * remain available for the rare path.
 *
 * Example:
 *   const rename = useApiMutation({
 *     mutationFn: (vars: { id: string; name: string }) =>
 *       api.api.tags({ id: vars.id }).patch({ name: vars.name }),
 *     invalidate: ["tags"],
 *     successToast: "Tag renamed",
 *     errorToast: "Failed to rename tag",
 *   });
 */
export function useApiMutation<TData, TVariables = void>(
    options: UseApiMutationOptions<TData, TVariables>,
) {
    const queryClient = useQueryClient();
    const { mutationFn, invalidate, successToast, errorToast, onSuccess, onError, ...rest } = options;

    return useMutation<TData, Error, TVariables>({
        ...rest,
        mutationFn: async vars => unwrapEden(await mutationFn(vars)) as TData,
        onSuccess: (data, vars) => {
            if (invalidate) {
                const keys: QueryKey[] = Array.isArray(invalidate[0]) ? (invalidate as QueryKey[]) : [invalidate as QueryKey];
                for (const key of keys) queryClient.invalidateQueries({ queryKey: key });
            }
            if (successToast !== false && successToast !== undefined) {
                const msg = typeof successToast === "function" ? successToast(data, vars) : successToast;
                if (msg) toast.success(msg);
            }
            onSuccess?.(data, vars);
        },
        onError: (err, vars) => {
            if (errorToast !== false) {
                const msg =
                    typeof errorToast === "function"
                        ? errorToast(err, vars)
                        : errorToast ?? err.message ?? "Request failed";
                if (msg) toast.error(msg);
            }
            onError?.(err, vars);
        },
    });
}
