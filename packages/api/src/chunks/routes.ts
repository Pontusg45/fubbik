import { Effect } from "effect";
import { Elysia, t } from "elysia";
import type { Session } from "../context";
import { AuthError } from "../errors";
import * as chunkService from "./service";

function requireSession(ctx: unknown): Effect.Effect<NonNullable<Session>, AuthError> {
  const session = (ctx as unknown as { session: Session }).session;
  if (!session) return Effect.fail(new AuthError());
  return Effect.succeed(session);
}

export const chunkRoutes = new Elysia()
  .get(
    "/chunks",
    (ctx) =>
      Effect.runPromise(
        requireSession(ctx).pipe(
          Effect.flatMap((session) => chunkService.listChunks(session.user.id, ctx.query)),
        ),
      ),
    {
      query: t.Object({
        type: t.Optional(t.String()),
        search: t.Optional(t.String()),
        limit: t.Optional(t.String()),
        offset: t.Optional(t.String()),
      }),
    },
  )
  .get("/chunks/:id", (ctx) =>
    Effect.runPromise(
      requireSession(ctx).pipe(
        Effect.flatMap((session) => chunkService.getChunkDetail(ctx.params.id, session.user.id)),
      ),
    ),
  )
  .post(
    "/chunks",
    (ctx) =>
      Effect.runPromise(
        requireSession(ctx).pipe(
          Effect.flatMap((session) => chunkService.createChunk(session.user.id, ctx.body)),
          Effect.tap(() =>
            Effect.sync(() => {
              ctx.set.status = 201;
            }),
          ),
        ),
      ),
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
    (ctx) =>
      Effect.runPromise(
        requireSession(ctx).pipe(
          Effect.flatMap((session) =>
            chunkService.updateChunk(ctx.params.id, session.user.id, ctx.body),
          ),
        ),
      ),
    {
      body: t.Object({
        title: t.Optional(t.String()),
        content: t.Optional(t.String()),
        type: t.Optional(t.String()),
        tags: t.Optional(t.Array(t.String())),
      }),
    },
  )
  .delete("/chunks/:id", (ctx) =>
    Effect.runPromise(
      requireSession(ctx).pipe(
        Effect.flatMap((session) => chunkService.deleteChunk(ctx.params.id, session.user.id)),
        Effect.map(() => ({ message: "Deleted" })),
      ),
    ),
  );
