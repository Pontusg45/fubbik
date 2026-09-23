import { describe, expect, it, vi } from "vitest";

import { createClient, unwrapResponse } from "./client";
import { ApiError, ApiResponseParseError, NetworkError } from "./errors";

function jsonResponse(value: unknown, status = 200): Response {
    return new Response(JSON.stringify(value), {
        status,
        headers: { "content-type": "application/json" }
    });
}

describe("generated client transport", () => {
    it("encodes path and scalar query values", async () => {
        // Given
        const fetcher = vi.fn(async () => jsonResponse([]));
        const client = createClient("http://api.test/", { fetch: fetcher });

        // When
        await client.api.chunks({ id: "a/b" }).get({ query: { includeCompleted: true, limit: 25 } });

        // Then
        expect(fetcher).toHaveBeenCalledWith(
            "http://api.test/api/chunks/a%2Fb?includeCompleted=true&limit=25",
            expect.objectContaining({ method: "GET" })
        );
    });

    it("serializes typed request bodies", async () => {
        // Given
        const fetcher = vi.fn(async () => jsonResponse({ id: "run-1" }));
        const client = createClient("http://api.test", { fetch: fetcher, credentials: "omit" });

        // When
        await client.api.plans({ planId: "plan-1" }).board.runs.post({ handle: "worker" });

        // Then
        expect(fetcher).toHaveBeenCalledWith(
            "http://api.test/api/plans/plan-1/board/runs",
            expect.objectContaining({
                body: JSON.stringify({ handle: "worker" }),
                credentials: "omit",
                headers: { "content-type": "application/json" },
                method: "POST"
            })
        );
    });

    it("keeps HTTP failures in the response envelope", async () => {
        // Given
        const client = createClient("", { fetch: vi.fn(async () => jsonResponse({ message: "missing" }, 404)) });

        // When
        const response = await client.api.chunks({ id: "missing" }).get();

        // Then
        expect(response).toEqual({ data: null, error: { status: 404, value: { message: "missing" } } });
        expect(() => unwrapResponse(response)).toThrow(ApiError);
    });

    it("normalizes network and invalid JSON failures", async () => {
        // Given
        const networkClient = createClient("", { fetch: vi.fn(async () => Promise.reject(new TypeError("offline"))) });
        // When the operation is evaluated by the assertion.
        // Then
        await expect(networkClient.api.chunks.get()).rejects.toBeInstanceOf(NetworkError);

        const invalidClient = createClient("", {
            fetch: vi.fn(async () => new Response("invalid", { headers: { "content-type": "application/json" } }))
        });
        await expect(invalidClient.api.chunks.get()).rejects.toBeInstanceOf(ApiResponseParseError);
    });
});
