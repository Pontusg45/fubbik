import { Effect } from "effect";
import { Elysia } from "elysia";

import { requireSession } from "../require-session";
import * as statsService from "./service";

export const statsRoutes = new Elysia().get("/stats", ctx =>
    Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => statsService.getUserStats(session.user.id))))
);
