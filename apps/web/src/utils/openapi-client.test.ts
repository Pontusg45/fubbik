import { describe, expect, it, vi, beforeEach } from "vitest";

import { ApiError, ApiResponseParseError, isNetworkError, NetworkError } from "@/lib/api-errors";

import { unwrapEden } from "./eden";
import { createClient } from "./openapi-client";

describe("openapi client", () => {
    beforeEach(() => {
        vi.stubGlobal(
            "fetch",
            vi.fn(
                async () =>
                    new Response(JSON.stringify({ ok: true }), {
                        status: 200,
                        headers: { "content-type": "application/json" }
                    })
            )
        );
    });

    it("builds a same-origin path when base is empty", async () => {
        // Given
        const api = createClient("");
        // When
        await api.api.chunks.get();
        // Then
        expect(fetch).toHaveBeenCalledWith("/api/chunks", expect.objectContaining({ method: "GET", credentials: "include" }));
    });

    it("builds an absolute path from a configured base", async () => {
        // Given
        const api = createClient("http://x.test");
        // When
        await api.api.chunks.get();
        // Then
        expect(fetch).toHaveBeenCalledWith("http://x.test/api/chunks", expect.objectContaining({ method: "GET" }));
    });

    it("interpolates path params from a call segment", async () => {
        // Given
        const api = createClient("http://x.test");
        // When
        await api.api.chunks({ id: "abc" }).get();
        // Then
        expect(fetch).toHaveBeenCalledWith("http://x.test/api/chunks/abc", expect.objectContaining({ method: "GET" }));
    });

    it("percent-encodes path params", async () => {
        // Given
        const api = createClient("http://x.test");
        // When
        await api.api.chunks({ id: "a/b c" }).get();
        // Then
        expect(fetch).toHaveBeenCalledWith("http://x.test/api/chunks/a%2Fb%20c", expect.objectContaining({ method: "GET" }));
    });

    it("returns { data, error } like eden", async () => {
        // Given
        const api = createClient("");
        // When
        const res = await api.api.chunks.get();
        // Then
        expect(res).toEqual({ data: { ok: true }, error: null });
    });

    it("throws NetworkError when fetch fails", async () => {
        // Given the inline inputs and test fixtures.
        // When
        vi.stubGlobal(
            "fetch",
            vi.fn(async () => Promise.reject(new TypeError("Failed to fetch")))
        );
        const api = createClient("");
        // Then
        await expect(api.api.chunks.get()).rejects.toBeInstanceOf(NetworkError);
    });

    it("puts the payload in error on a failed response", async () => {
        // Given
        vi.stubGlobal(
            "fetch",
            vi.fn(
                async () =>
                    new Response(JSON.stringify({ message: "nope" }), {
                        status: 404,
                        headers: { "content-type": "application/json" }
                    })
            )
        );
        const api = createClient("");
        // When
        const res = await api.api.chunks.get();
        // Then
        expect(res.data).toBeNull();
        expect(res.error).toEqual({ status: 404, value: { message: "nope" } });
    });

    it("handles an empty successful response", async () => {
        // Given the inline inputs and test fixtures.
        // When
        vi.stubGlobal(
            "fetch",
            vi.fn(async () => new Response(null, { status: 204 }))
        );
        const api = createClient("");
        // Then
        await expect(api.api.chunks.get()).resolves.toEqual({ data: null, error: null });
    });

    it("surfaces malformed JSON as a contract failure", async () => {
        // Given the inline inputs and test fixtures.
        // When
        vi.stubGlobal(
            "fetch",
            vi.fn(async () => new Response("not-json", { status: 200, headers: { "content-type": "application/json" } }))
        );
        const api = createClient("");
        // Then
        await expect(api.api.chunks.get()).rejects.toBeInstanceOf(ApiResponseParseError);
    });
});

describe("api-errors", () => {
    it("detects network failures", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(isNetworkError(new NetworkError())).toBe(true);
        expect(isNetworkError(new TypeError("Failed to fetch"))).toBe(false);
    });

    it("unwrapEden throws ApiError with server message", () => {
        // Given an error response with a server message.
        try {
            // When
            unwrapEden({ data: null, error: { status: 404, value: { message: "missing" } } });
        } catch (error) {
            // Then
            expect(error).toBeInstanceOf(ApiError);
            expect((error as ApiError).status).toBe(404);
            expect((error as ApiError).message).toBe("missing");
        }
    });
});
