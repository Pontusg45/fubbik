import { Effect } from "effect";
import { Elysia } from "elysia";

import { optionalSession } from "../require-session";
import * as graphService from "./service";

export const graphRoutes = new Elysia().get("/graph", ctx =>
    Effect.runPromise(optionalSession(ctx).pipe(Effect.flatMap(session => graphService.getUserGraph(session?.user.id))))
);
