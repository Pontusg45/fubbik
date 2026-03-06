import { Elysia } from "elysia";
import { auth } from "@fubbik/auth";
import type { Session } from "./context";
import { healthRoutes } from "./health/routes";
import { chunkRoutes } from "./chunks/routes";
import { statsRoutes } from "./stats/routes";

const isDev = process.env.NODE_ENV !== "production";

const DEV_USER_ID = "dev-user";
const DEV_SESSION: Session = {
  session: {
    id: "dev-session",
    token: "dev-token",
    userId: DEV_USER_ID,
    expiresAt: new Date(Date.now() + 86400000),
    createdAt: new Date(),
    updatedAt: new Date(),
    ipAddress: null,
    userAgent: null,
  },
  user: {
    id: DEV_USER_ID,
    name: "Dev User",
    email: "dev@localhost",
    emailVerified: false,
    image: null,
    createdAt: new Date(),
    updatedAt: new Date(),
  },
};

async function getSession(headers: Headers): Promise<Session> {
  const session = await auth.api.getSession({ headers });
  if (!session && isDev) return DEV_SESSION;
  return session;
}

export const api = new Elysia({ prefix: "/api" })
  .use(healthRoutes)
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
  })
  .use(chunkRoutes)
  .use(statsRoutes);

export type Api = typeof api;
