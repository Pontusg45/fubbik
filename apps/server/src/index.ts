import { cors } from "@elysiajs/cors";
import { swagger } from "@elysiajs/swagger";
import { api } from "@fubbik/api";
import { auth } from "@fubbik/auth";
import { env } from "@fubbik/env/server";
import { Elysia } from "elysia";
import { rateLimit } from "elysia-rate-limit";

import { logger } from "./logger";

new Elysia()
    .use(
        swagger({
            path: "/docs",
            documentation: {
                info: { title: "Fubbik API", version: "0.1.0" }
            }
        })
    )
    .use(
        rateLimit({
            max: Number(env.RATE_LIMIT_MAX ?? "100"),
            duration: Number(env.RATE_LIMIT_DURATION_MS ?? "60000")
        })
    )
    .use(
        cors({
            origin: env.CORS_ORIGIN.includes(",") ? env.CORS_ORIGIN.split(",").map(s => s.trim()) : env.CORS_ORIGIN,
            methods: ["GET", "POST", "PATCH", "DELETE", "OPTIONS"],
            allowedHeaders: ["Content-Type", "Authorization"],
            credentials: true
        })
    )
    .onRequest(({ request }) => {
        const url = new URL(request.url);
        logger.info(`${request.method} ${url.pathname}`);

        // Handle auth routes before Elysia parses the request body,
        // which would consume the body stream and break better-auth
        if (url.pathname.startsWith("/api/auth")) {
            return auth.handler(request);
        }
    })
    .onError(({ error, request }) => {
        const pathname = new URL(request.url).pathname;
        logger.error(`${request.method} ${pathname}`, {
            error: "message" in error ? error.message : String(error)
        });
    })
    .use(api)
    .get("/", () => "OK")
    .listen(Number(env.PORT), () => {
        logger.info(`Server is running on http://localhost:${env.PORT}`);
    });
