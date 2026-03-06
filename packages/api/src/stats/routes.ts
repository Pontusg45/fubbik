import { Elysia } from "elysia";
import { dbError } from "../error";
import * as statsService from "./service";

export const statsRoutes = new Elysia().get("/stats", async ({ session, set }) => {
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
