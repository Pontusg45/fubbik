import { Effect } from "effect";
import { Elysia } from "elysia";

import { optionalSession } from "../require-session";
import * as tagService from "./service";

export const tagRoutes = new Elysia().get("/tags", ctx =>
    Effect.runPromise(optionalSession(ctx).pipe(Effect.flatMap(session => tagService.getUserTags(session?.user.id))))
);
