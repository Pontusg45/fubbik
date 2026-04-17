import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as tagTypeService from "./service";

export const tagTypeRoutes = new Elysia()
    .get("/tag-types", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => tagTypeService.listTagTypes(session.user.id))))
    )
    .post(
        "/tag-types",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => tagTypeService.createTagType(session.user.id, ctx.body)),
                    Effect.tap(() => Effect.sync(() => { ctx.set.status = 201; }))
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 50 }),
                color: t.Optional(t.String({ maxLength: 7 })),
                icon: t.Optional(t.Union([t.String({ maxLength: 40 }), t.Null()]))
            })
        }
    )
    .patch(
        "/tag-types/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => tagTypeService.updateTagType(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 50 })),
                color: t.Optional(t.String({ maxLength: 7 })),
                icon: t.Optional(t.Union([t.String({ maxLength: 40 }), t.Null()]))
            })
        }
    )
    .delete("/tag-types/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => tagTypeService.deleteTagType(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
