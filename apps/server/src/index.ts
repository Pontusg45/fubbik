import { cors } from "@elysiajs/cors";
import { swagger } from "@elysiajs/swagger";
import { api, awaitImplicitDevUserBootstrap, initStartupTasks } from "@fubbik/api";
import { auth } from "@fubbik/auth";
import { env } from "@fubbik/env/server";
import { Elysia } from "elysia";
import { rateLimit } from "elysia-rate-limit";

import { startTracing, shutdownTracing } from "./lib/tracing";
import { logger } from "./logger";

// Start OpenTelemetry if an OTLP endpoint is configured
if (process.env.OTEL_EXPORTER_OTLP_ENDPOINT) {
    startTracing();
}

await awaitImplicitDevUserBootstrap();

const server = new Elysia()
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
        const message = "message" in error ? String(error.message) : String(error);
        // Effect.runPromise throws FiberFailure with generic message — log full string for context.
        const errorLog = message === "An error has occurred" ? `${(error as Error).name}: ${String(error)}` : message;
        logger.error(`${request.method} ${pathname}`, { error: errorLog });
    })
    .use(api)
    .get("/", () => "OK")
    .listen(Number(env.PORT), () => {
        logger.info(`Server is running on http://localhost:${env.PORT}`);
        initStartupTasks();
    });

let shuttingDown = false;
async function shutdown(signal: string) {
    if (shuttingDown) return;
    shuttingDown = true;
    logger.info(`${signal} received; draining server`);
    await Promise.race([
        (async () => {
            await server.stop();
            if (process.env.OTEL_EXPORTER_OTLP_ENDPOINT) {
                await shutdownTracing();
            }
        })(),
        Bun.sleep(10_000).then(() => logger.warn("Shutdown drain deadline exceeded"))
    ]);
    process.exit(0);
}

process.on("SIGTERM", () => void shutdown("SIGTERM"));
process.on("SIGINT", () => void shutdown("SIGINT"));
