import { env } from "@fubbik/env/web";

/** Mirrors API dev-user fallback so the shell can show an account menu without Better Auth cookie. */
export const IMPLICIT_DEV_USER_MENU = {
    id: "dev-user",
    name: "Dev User",
    email: "dev@localhost"
} as const;

/**
 * Hide Sign-In /implicit session UX: Vite dev, or Docker self-host build (`VITE_FUBBIK_IMPLICIT_DEV_SESSION=true`).
 * Disabled when `VITE_FUBBIK_DISABLE_IMPLICIT_DEV_UX=true` (Playwright).
 */
export function isImplicitDevUxEnabled(): boolean {
    if (import.meta.env.VITE_FUBBIK_DISABLE_IMPLICIT_DEV_UX === "true") return false;

    const buildImplicit =
        env.VITE_FUBBIK_IMPLICIT_DEV_SESSION === "true" || import.meta.env.VITE_FUBBIK_IMPLICIT_DEV_SESSION === "true";

    return Boolean(import.meta.env.DEV || buildImplicit);
}
