import { Elysia } from "elysia";

import { auth } from "@fubbik/auth";

import type { Session } from "./context";

function getSession(headers: Headers): Promise<Session> {
  return auth.api.getSession({ headers });
}

export const api = new Elysia({ prefix: "/api" })
  .get("/health", () => "OK")
  .resolve(async ({ headers }) => {
    const session = await getSession(new Headers(headers as Record<string, string>));
    return { session };
  })
  .get("/me", ({ session, set }) => {
    if (!session) {
      set.status = 401;
      return { message: "Authentication required" };
    }
    return { message: "This is private", user: session.user };
  });

export type Api = typeof api;
