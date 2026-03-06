import { Elysia, t } from "elysia";
import { dbError } from "../error";
import * as chunkService from "./service";

export const chunkRoutes = new Elysia()
  .get(
    "/chunks",
    async ({ session, set, query }) => {
      if (!session) {
        set.status = 401;
        return { message: "Authentication required" };
      }
      try {
        return await chunkService.listChunks(session.user.id, query);
      } catch (err) {
        return dbError(set, "Failed to fetch chunks", err);
      }
    },
    {
      query: t.Object({
        type: t.Optional(t.String()),
        search: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        offset: t.Optional(t.String()),
      }),
    },
  )
  .get("/chunks/:id", async ({ session, set, params }) => {
    if (!session) {
      set.status = 401;
      return { message: "Authentication required" };
    }
    try {
      const result = await chunkService.getChunkDetail(params.id, session.user.id);
      if (!result) {
        set.status = 404;
        return { message: "Chunk not found" };
      }
      return result;
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
        const created = await chunkService.createChunk(session.user.id, body);
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
        const updated = await chunkService.updateChunk(params.id, session.user.id, body);
        if (!updated) {
          set.status = 404;
          return { message: "Chunk not found" };
        }
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
      const deleted = await chunkService.deleteChunk(params.id, session.user.id);
      if (!deleted) {
        set.status = 404;
        return { message: "Chunk not found" };
      }
      return { message: "Deleted" };
    } catch (err) {
      return dbError(set, "Failed to delete chunk", err);
    }
  });
