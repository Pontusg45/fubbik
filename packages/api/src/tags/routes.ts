import { Effect } from "effect";
import { Elysia } from "elysia";

import { requireSession } from "../require-session";
import * as tagService from "./service";

export const tagRoutes = new Elysia().get("/tags", ctx =>
    Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => tagService.getUserTags(session.user.id))))
);
