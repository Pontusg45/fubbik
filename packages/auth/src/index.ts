import { db } from "@fubbik/db";
import * as schema from "@fubbik/db/schema/auth";
import { env } from "@fubbik/env/server";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";

const relaxedHttpAuth = env.NODE_ENV !== "production" || env.FUBBIK_IMPLICIT_DEV_SESSION === "true";

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
        defaultCookieAttributes: {
            sameSite: relaxedHttpAuth ? "lax" : "none",
            secure: !relaxedHttpAuth,
            httpOnly: true
        }
    },
    plugins: []
});
