import { treaty } from "@elysiajs/eden";
import type { Api } from "@fubbik/api";

import { asNetworkError } from "@/lib/api-errors";
import { apiOrigin, legacyApiOrigin } from "@/lib/api-origin";

import { createClient } from "./openapi-client";

// `api` — Rust OpenAPI client (primary).
// `legacyApi` — Eden treaty for routes not yet in openapi.json (see
// `./legacy-api-routes.ts`). It stays on a separate origin until those
// routes are ported to Rust.

export const api = createClient(apiOrigin());

const LEGACY_REQUEST_TIMEOUT_MS = 30_000;
const legacyFetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const timeout = AbortSignal.timeout(LEGACY_REQUEST_TIMEOUT_MS);
    const signal = init?.signal ? AbortSignal.any([init.signal, timeout]) : timeout;
    try {
        return await fetch(input, { ...init, signal });
    } catch (error) {
        throw asNetworkError(error);
    }
};

export const legacyApi = treaty<Api>(legacyApiOrigin(), {
    fetch: { credentials: "include" },
    // Bun augments `typeof fetch` with a static `preconnect` method; Eden's
    // fetcher only invokes the callable part, which this wrapper preserves.
    fetcher: legacyFetch as typeof fetch
});
