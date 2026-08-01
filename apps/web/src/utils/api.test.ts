import { describe, expect, it, vi, beforeEach } from "vitest";
import { createClient } from "./api";

describe("proxy api client", () => {
    beforeEach(() => {
        vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ ok: true }), {
            status: 200,
            headers: { "content-type": "application/json" }
        })));
    });

    it("builds a path from property access", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks.get();
        expect(fetch).toHaveBeenCalledWith(
            "http://x.test/api/chunks",
            expect.objectContaining({ method: "GET", credentials: "include" })
        );
    });

    it("interpolates path params from a call segment", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks({ id: "abc" }).get();
        expect(fetch).toHaveBeenCalledWith(
            "http://x.test/api/chunks/abc",
            expect.objectContaining({ method: "GET" })
        );
    });

    it("converts camelCase segments to kebab-case paths", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks({ id: "abc" })["applies-to"].get();
        expect(fetch).toHaveBeenCalledWith(
            "http://x.test/api/chunks/abc/applies-to",
            expect.anything()
        );
    });

    it("sends a JSON body on post", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks.post({ title: "T" });
        expect(fetch).toHaveBeenCalledWith(
            "http://x.test/api/chunks",
            expect.objectContaining({
                method: "POST",
                body: JSON.stringify({ title: "T" })
            })
        );
    });

    it("serialises query params", async () => {
        const api = createClient("http://x.test");
        await api.api.chunks.get({ query: { type: "note", limit: "10" } });
        expect(fetch).toHaveBeenCalledWith(
            "http://x.test/api/chunks?type=note&limit=10",
            expect.anything()
        );
    });

    it("returns { data, error } like eden", async () => {
        const api = createClient("http://x.test");
        const res = await api.api.chunks.get();
        expect(res).toEqual({ data: { ok: true }, error: null });
    });

    it("puts the payload in error on a failed response", async () => {
        vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ message: "nope" }), {
            status: 404,
            headers: { "content-type": "application/json" }
        })));
        const api = createClient("http://x.test");
        const res = await api.api.chunks.get();
        expect(res.data).toBeNull();
        expect(res.error).toEqual({ status: 404, value: { message: "nope" } });
    });
});
