import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as contextForFileService from "./service";

export const contextForFileRoutes = new Elysia().get(
    "/context/for-file",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    contextForFileService.getContextForFile(
                        session.user.id,
                        ctx.query.path,
                        ctx.query.codebaseId
                    )
                )
            )
        ),
    {
        query: t.Object({
            path: t.String(),
            codebaseId: t.Optional(t.String())
        })
    }
);
