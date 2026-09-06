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
        const api = createClient("");
        await api.api.chunks.get();
        expect(fetch).toHaveBeenCalledWith("/api/chunks", expect.objectContaining({ method: "GET", credentials: "include" }));
    });

    it("builds an absolute path from a configured base", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks.get();
        expect(fetch).toHaveBeenCalledWith("http://x.test/api/chunks", expect.objectContaining({ method: "GET" }));
    });

    it("interpolates path params from a call segment", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks({ id: "abc" }).get();
        expect(fetch).toHaveBeenCalledWith("http://x.test/api/chunks/abc", expect.objectContaining({ method: "GET" }));
    });

    it("percent-encodes path params", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks({ id: "a/b c" }).get();
        expect(fetch).toHaveBeenCalledWith("http://x.test/api/chunks/a%2Fb%20c", expect.objectContaining({ method: "GET" }));
    });

    it("returns { data, error } like eden", async () => {
        const api = createClient("");
        const res = await api.api.chunks.get();
        expect(res).toEqual({ data: { ok: true }, error: null });
    });

    it("throws NetworkError when fetch fails", async () => {
        vi.stubGlobal(
            "fetch",
            vi.fn(async () => Promise.reject(new TypeError("Failed to fetch")))
        );
        const api = createClient("");
        await expect(api.api.chunks.get()).rejects.toBeInstanceOf(NetworkError);
    });

    it("puts the payload in error on a failed response", async () => {
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
        const res = await api.api.chunks.get();
        expect(res.data).toBeNull();
        expect(res.error).toEqual({ status: 404, value: { message: "nope" } });
    });

    it("handles an empty successful response", async () => {
        vi.stubGlobal(
            "fetch",
            vi.fn(async () => new Response(null, { status: 204 }))
        );
        const api = createClient("");
        await expect(api.api.chunks.get()).resolves.toEqual({ data: null, error: null });
    });

    it("surfaces malformed JSON as a contract failure", async () => {
        vi.stubGlobal(
            "fetch",
            vi.fn(async () => new Response("not-json", { status: 200, headers: { "content-type": "application/json" } }))
        );
        const api = createClient("");
        await expect(api.api.chunks.get()).rejects.toBeInstanceOf(ApiResponseParseError);
    });
});

describe("api-errors", () => {
    it("detects network failures", () => {
        expect(isNetworkError(new NetworkError())).toBe(true);
        expect(isNetworkError(new TypeError("Failed to fetch"))).toBe(false);
    });

    it("unwrapEden throws ApiError with server message", () => {
        try {
            unwrapEden({ data: null, error: { status: 404, value: { message: "missing" } } });
        } catch (error) {
            expect(error).toBeInstanceOf(ApiError);
            expect((error as ApiError).status).toBe(404);
            expect((error as ApiError).message).toBe("missing");
        }
    });
});
