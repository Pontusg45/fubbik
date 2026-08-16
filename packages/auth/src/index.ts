import { db } from "@fubbik/db";
import * as schema from "@fubbik/db/schema/auth";
import { env } from "@fubbik/env/server";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";

const relaxedHttpAuth = env.NODE_ENV !== "production" || env.FUBBIK_IMPLICIT_DEV_SESSION === "true";

/**
 * The session cookie has to reach two origins: Node (which issues it) and the
 * Rust API (which only verifies it). Under the Caddy setup those are sibling
 * subdomains — `api.fubbik.test` and `rs.fubbik.test` — and a cookie with no
 * `Domain` attribute is host-only per RFC 6265, so it would never be sent to
 * the second one.
 *
 * Returns `undefined` unless the host genuinely has a parent domain to share:
 *
 *   - `localhost` — no parent, and none needed. Cookies are NOT isolated by
 *     port, so Node on :3000 and Rust on :3100 already share them. Setting
 *     `Domain=localhost` here would be worse than useless: browsers reject it.
 *   - a bare IP — `127.0.0.1` has four labels but no registrable parent;
 *     deriving `.0.1` from it would silently break auth.
 */
function crossSubDomainCookies() {
    let hostname: string;
    try {
        hostname = new URL(env.BETTER_AUTH_URL).hostname;
    } catch {
        return undefined;
    }

    const labels = hostname.split(".");
    const isIpLiteral = labels.every(l => /^\d+$/.test(l)) || hostname.includes(":");
    if (isIpLiteral || labels.length < 3) {
        return undefined;
    }

    return { enabled: true as const, domain: `.${labels.slice(-2).join(".")}` };
}

export const auth = betterAuth({
    database: drizzleAdapter(db, {
        provider: "pg",

        schema: schema
    }),
    trustedOrigins: env.CORS_ORIGIN.includes(",") ? env.CORS_ORIGIN.split(",").map(s => s.trim()) : [env.CORS_ORIGIN],
    emailAndPassword: {
        enabled: true
    },
    advanced: {
        crossSubDomainCookies: crossSubDomainCookies(),
        defaultCookieAttributes: {
            sameSite: relaxedHttpAuth ? "lax" : "none",
            secure: !relaxedHttpAuth,
            httpOnly: true
        }
    },
    plugins: []
});
