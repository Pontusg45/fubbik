import { cors } from "@elysiajs/cors";
import { api } from "@fubbik/api";
import { auth } from "@fubbik/auth";
import { env } from "@fubbik/env/server";
import { Elysia } from "elysia";

new Elysia()
  .use(
    cors({
      origin: env.CORS_ORIGIN,
      methods: ["GET", "POST", "PATCH", "DELETE", "OPTIONS"],
      allowedHeaders: ["Content-Type", "Authorization"],
      credentials: true,
    }),
  )
  .all("/api/auth/*", async (context) => {
    const { request, status } = context;
    if (["POST", "GET"].includes(request.method)) {
      return auth.handler(request);
    }
    return status(405);
  })
  .use(api)
  .get("/", () => "OK")
  .listen(Number(env.PORT), () => {
    console.log(`Server is running on http://localhost:${env.PORT}`);
  });
