/**
 * Unwrap an Eden treaty response, throwing on error.
 * Removes the need for `as Exclude<typeof data, { message: string }>` everywhere.
 */
export function unwrapEden<T>(response: { data: T | { message: string } | null; error: unknown }): T {
    if (response.error) throw new Error("Request failed");
    return response.data as T;
}
