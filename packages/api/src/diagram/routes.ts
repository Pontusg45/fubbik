import { Effect } from "effect";
import { Elysia } from "elysia";

import { requireSession } from "../require-session";
import * as diagramService from "./service";

export const diagramRoutes = new Elysia().get(
    "/codebases/:id/diagram",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => diagramService.generateDiagram(session.user.id, ctx.params.id))
            )
        )
);
