import { Effect } from "effect";
import { Elysia } from "elysia";
import type { Session } from "../context";
import { AuthError } from "../errors";
import * as statsService from "./service";

function requireSession(ctx: unknown) {
  const session = (ctx as unknown as { session: Session }).session;
  return session ? Effect.succeed(session) : Effect.fail(new AuthError());
}

export const statsRoutes = new Elysia().get("/stats", (ctx) =>
  Effect.runPromise(
    requireSession(ctx).pipe(
      Effect.flatMap((session) => statsService.getUserStats(session.user.id)),
    ),
  ),
);
