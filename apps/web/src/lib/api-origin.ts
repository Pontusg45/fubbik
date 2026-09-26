import { env } from "@fubbik/env/web";

/** Default API port when `just dev` runs the Rust server. */
const DEV_API_PORT = 3000;

const SERVER_DEFAULT = `http://127.0.0.1:${DEV_API_PORT}`;

/**
 * Resolved API origin for HTTP clients.
 *
 * - **Browser, production:** same-origin `/api/...` served by the Rust process.
 * - **Browser, local dev:** configured origin, or same-origin through the Vite proxy.
 * - **Static-shell build:** a loopback origin is only used while rendering the shell.
 */
export function apiOrigin(): string {
    if (import.meta.env.PROD && typeof window !== "undefined") return "";

    const configured = (env.VITE_API_ORIGIN ?? env.VITE_API_URL ?? env.VITE_SERVER_URL)?.replace(/\/$/, "");
    if (configured) return configured;

    if (typeof window !== "undefined") return "";
    return SERVER_DEFAULT;
}

/** Absolute base for clients that require a full URL. */
export function apiBaseUrl(): string {
    const origin = apiOrigin();
    if (origin) return origin;
    if (typeof window !== "undefined") return window.location.origin;
    return SERVER_DEFAULT;
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
