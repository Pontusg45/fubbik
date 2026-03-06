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
            max: 100,
            duration: 60_000
        })
    )
    .use(
        cors({
            origin: env.CORS_ORIGIN,
            methods: ["GET", "POST", "PATCH", "DELETE", "OPTIONS"],
            allowedHeaders: ["Content-Type", "Authorization"],
            credentials: true
        })
    )
    .onRequest(({ request }) => {
        logger.info(`${request.method} ${new URL(request.url).pathname}`);
    })
    .onError(({ error, request }) => {
        logger.error(`${request.method} ${new URL(request.url).pathname}`, {
            error: "message" in error ? error.message : String(error)
        });
    })
    .all("/api/auth/*", async context => {
        const { request, status } = context;
        if (["POST", "GET"].includes(request.method)) {
            return auth.handler(request);
        }
        return status(405);
    })
    .use(api)
    .get("/", () => "OK")
    .listen(Number(env.PORT), () => {
        logger.info(`Server is running on http://localhost:${env.PORT}`);
    });
