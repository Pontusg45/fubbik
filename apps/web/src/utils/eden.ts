import { ApiError, isNetworkError } from "@/lib/api-errors";

/**
 * Unwrap an Eden or OpenAPI client response, throwing typed errors on failure.
 */
export function unwrapEden<TData>(response: { data: TData; error: unknown }): Exclude<TData, { message: string } | null> {
    if (response.error) {
        const err = response.error as { status?: number; value?: unknown };
        if (typeof err.status === "number") {
            throw new ApiError(err.status, err.value);
        }
        throw new ApiError(0, err, "Request failed");
    }
    return response.data as Exclude<TData, { message: string } | null>;
}

export { isNetworkError, ApiError };
