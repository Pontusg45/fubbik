import { and, desc, eq, ilike, or, sql } from "drizzle-orm";
import { Elysia, t } from "elysia";

import { auth } from "@fubbik/auth";
import { db } from "@fubbik/db";
import { chunk, chunkConnection } from "@fubbik/db/schema/chunk";

import type { Session } from "./context";
import { dbError } from "./error";

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

function generateId() {
  return crypto.randomUUID();
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
  })
  // --- Chunks CRUD ---
  .get(
    "/chunks",
    async ({ session, set, query }) => {
      if (!session) {
        set.status = 401;
        return { message: "Authentication required" };
      }
      try {
        const conditions = [eq(chunk.userId, session.user.id)];
        if (query.type) {
          conditions.push(eq(chunk.type, query.type));
        }
        if (query.search) {
          conditions.push(
            or(ilike(chunk.title, `%${query.search}%`), ilike(chunk.content, `%${query.search}%`))!,
          );
        }
        const chunks = await db
          .select()
          .from(chunk)
          .where(and(...conditions))
          .orderBy(desc(chunk.updatedAt))
          .limit(query.limit ? Number(query.limit) : 50);

        const total = await db
          .select({ count: sql<number>`count(*)` })
          .from(chunk)
          .where(eq(chunk.userId, session.user.id));

        return { chunks, total: Number(total[0]?.count ?? 0) };
      } catch (err) {
        return dbError(set, "Failed to fetch chunks", err);
      }
    },
    {
      query: t.Object({
        type: t.Optional(t.String()),
        search: t.Optional(t.String()),
        limit: t.Optional(t.String()),
      }),
    },
  )
  .get("/chunks/:id", async ({ session, set, params }) => {
    if (!session) {
      set.status = 401;
      return { message: "Authentication required" };
    }
    try {
      const [found] = await db
        .select()
        .from(chunk)
        .where(and(eq(chunk.id, params.id), eq(chunk.userId, session.user.id)));
      if (!found) {
        set.status = 404;
        return { message: "Chunk not found" };
      }
      const connections = await db
        .select({
          id: chunkConnection.id,
          targetId: chunkConnection.targetId,
          sourceId: chunkConnection.sourceId,
          relation: chunkConnection.relation,
          title: chunk.title,
        })
        .from(chunkConnection)
        .leftJoin(
          chunk,
          or(
            and(eq(chunkConnection.targetId, chunk.id), eq(chunkConnection.sourceId, params.id)),
            and(eq(chunkConnection.sourceId, chunk.id), eq(chunkConnection.targetId, params.id)),
          ),
        )
        .where(or(eq(chunkConnection.sourceId, params.id), eq(chunkConnection.targetId, params.id)));

      return { chunk: found, connections };
    } catch (err) {
      return dbError(set, "Failed to fetch chunk", err);
    }
  })
  .post(
    "/chunks",
    async ({ session, set, body }) => {
      if (!session) {
        set.status = 401;
        return { message: "Authentication required" };
      }
      try {
        const id = generateId();
        const [created] = await db
          .insert(chunk)
          .values({
            id,
            title: body.title,
            content: body.content ?? "",
            type: body.type ?? "note",
            tags: body.tags ?? [],
            userId: session.user.id,
          })
          .returning();
        set.status = 201;
        return created;
      } catch (err) {
        return dbError(set, "Failed to create chunk", err);
      }
    },
    {
      body: t.Object({
        title: t.String(),
        content: t.Optional(t.String()),
        type: t.Optional(t.String()),
        tags: t.Optional(t.Array(t.String())),
      }),
    },
  )
  .patch(
    "/chunks/:id",
    async ({ session, set, params, body }) => {
      if (!session) {
        set.status = 401;
        return { message: "Authentication required" };
      }
      try {
        const [existing] = await db
          .select()
          .from(chunk)
          .where(and(eq(chunk.id, params.id), eq(chunk.userId, session.user.id)));
        if (!existing) {
          set.status = 404;
          return { message: "Chunk not found" };
        }
        const [updated] = await db
          .update(chunk)
          .set({
            ...(body.title !== undefined && { title: body.title }),
            ...(body.content !== undefined && { content: body.content }),
            ...(body.type !== undefined && { type: body.type }),
            ...(body.tags !== undefined && { tags: body.tags }),
          })
          .where(eq(chunk.id, params.id))
          .returning();
        return updated;
      } catch (err) {
        return dbError(set, "Failed to update chunk", err);
      }
    },
    {
      body: t.Object({
        title: t.Optional(t.String()),
        content: t.Optional(t.String()),
        type: t.Optional(t.String()),
        tags: t.Optional(t.Array(t.String())),
      }),
    },
  )
  .delete("/chunks/:id", async ({ session, set, params }) => {
    if (!session) {
      set.status = 401;
      return { message: "Authentication required" };
    }
    try {
      const [deleted] = await db
        .delete(chunk)
        .where(and(eq(chunk.id, params.id), eq(chunk.userId, session.user.id)))
        .returning();
      if (!deleted) {
        set.status = 404;
        return { message: "Chunk not found" };
      }
      return { message: "Deleted" };
    } catch (err) {
      return dbError(set, "Failed to delete chunk", err);
    }
  })
  // --- Stats ---
  .get("/stats", async ({ session, set }) => {
    if (!session) {
      set.status = 401;
      return { message: "Authentication required" };
    }
    try {
      const userId = session.user.id;
      const [chunkCount] = await db
        .select({ count: sql<number>`count(*)` })
        .from(chunk)
        .where(eq(chunk.userId, userId));
      const [connectionCount] = await db
        .select({ count: sql<number>`count(*)` })
        .from(chunkConnection)
        .innerJoin(chunk, eq(chunkConnection.sourceId, chunk.id))
        .where(eq(chunk.userId, userId));
      const [tagCount] = await db
        .select({
          count: sql<number>`count(distinct tag)`,
        })
        .from(
          sql`(select jsonb_array_elements_text(${chunk.tags}) as tag from ${chunk} where ${chunk.userId} = ${userId}) t`,
        );

      return {
        chunks: Number(chunkCount?.count ?? 0),
        connections: Number(connectionCount?.count ?? 0),
        tags: Number(tagCount?.count ?? 0),
      };
    } catch (err) {
      return dbError(set, "Failed to fetch stats", err);
    }
  });

export type Api = typeof api;
