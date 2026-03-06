/**
 * Unwrap an Eden treaty response, throwing on error.
 * Removes the need for `as Exclude<typeof data, { message: string }>` everywhere.
 */
export function unwrapEden<TData>(response: {
    data: TData;
    error: unknown;
}): Exclude<TData, { message: string } | null> {
    if (response.error) throw new Error("Request failed");
    return response.data as Exclude<TData, { message: string } | null>;
}
