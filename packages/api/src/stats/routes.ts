import { Elysia } from "elysia";
import type { Session } from "../context";
import { dbError } from "../error";
import * as statsService from "./service";

export const statsRoutes = new Elysia().get("/stats", async (ctx) => {
  const { set } = ctx;
  const session = (ctx as unknown as { session: Session }).session;
  if (!session) {
    set.status = 401;
    return { message: "Authentication required" };
  }
  try {
    return await statsService.getUserStats(session.user.id);
  } catch (err) {
    return dbError(set, "Failed to fetch stats", err);
  }
});
