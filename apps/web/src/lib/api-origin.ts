import { env } from "@fubbik/env/web";

/** Default API port when `just dev` runs the Rust server. */
const DEV_API_PORT = 3000;

const SSR_DEFAULT = `http://127.0.0.1:${DEV_API_PORT}`;

/**
 * Resolved API origin for HTTP clients.
 *
 * - **Browser, local dev:** empty string → same-origin `/api/...` (Vite proxy).
 * - **Browser, production:** empty when unset → same-origin behind reverse proxy.
 * - **SSR / server-side fetch:** absolute URL (`SSR_API_ORIGIN`, configured origin, or loopback default).
 */
export function apiOrigin(): string {
    if (typeof window === "undefined") {
        const ssr =
            (typeof process !== "undefined" ? process.env.SSR_API_ORIGIN : undefined) ??
            (import.meta as ImportMeta & { env?: { SSR_API_ORIGIN?: string } }).env?.SSR_API_ORIGIN;
        if (ssr) return ssr.replace(/\/$/, "");
    }

    const configured = (env.VITE_API_ORIGIN ?? env.VITE_API_URL ?? env.VITE_SERVER_URL)?.replace(/\/$/, "");
    if (configured) return configured;

    if (typeof window !== "undefined") return "";
    return SSR_DEFAULT;
}

/** Absolute base for clients that require a full URL. */
export function apiBaseUrl(): string {
    const origin = apiOrigin();
    if (origin) return origin;
    if (typeof window !== "undefined") return window.location.origin;
    return SSR_DEFAULT;
}

/** Build an absolute or same-origin API path. */
export function apiUrl(path: string): string {
    const normalized = path.startsWith("/") ? path : `/${path}`;
    const origin = apiOrigin();
    return origin ? `${origin}${normalized}` : normalized;
}

/** Log API wiring once in dev; warn when deprecated env vars disagree. */
export function logApiOriginInDev(): void {
    if (!import.meta.env.DEV || typeof window === "undefined") return;

    const origin = apiOrigin();
    const mode = origin ? `absolute → ${origin}` : "same-origin /api (Vite proxy or reverse proxy)";
    console.info(`[fubbik] API ${mode}`);

    const server = env.VITE_SERVER_URL;
    const api = env.VITE_API_URL;
    if (server && api && server !== api) {
        console.warn(
            "[fubbik] VITE_SERVER_URL and VITE_API_URL differ — prefer a single VITE_API_ORIGIN or leave all unset for same-origin /api"
        );
    }
}
