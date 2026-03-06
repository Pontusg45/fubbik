import { Elysia } from "elysia";
import { checkDbConnectivity } from "@fubbik/db/repository";

export const healthRoutes = new Elysia().get("/health", async ({ set }) => {
  try {
    await checkDbConnectivity();
    return { status: "ok", db: "connected" };
  } catch {
    set.status = 503;
    return { status: "degraded", db: "disconnected" };
  }
});
