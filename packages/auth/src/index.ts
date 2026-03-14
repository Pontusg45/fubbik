import { db } from "@fubbik/db";
import * as schema from "@fubbik/db/schema/auth";
import { env } from "@fubbik/env/server";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";

const isDev = process.env.NODE_ENV !== "production";

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
            sameSite: isDev ? "lax" : "none",
            secure: !isDev,
            httpOnly: true
        }
    },
    plugins: []
});
