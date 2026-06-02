import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as documentService from "./service";

export const documentRoutes = new Elysia()
    .get(
        "/documents",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => documentService.listDocumentsWithTags(session.user.id, ctx.query.spaceId))
                )
            ),
        {
            query: t.Object({
                spaceId: t.Optional(t.String())
            })
        }
    )
    .get(
        "/documents/search",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => documentService.searchDocuments(session.user.id, ctx.query.q, ctx.query.spaceId))
                )
            ),
        {
            query: t.Object({
                q: t.String({ minLength: 2 }),
                spaceId: t.Optional(t.String())
            })
        }
    )
    .get(
        "/documents/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => documentService.getDocument(ctx.params.id, session.user.id)))
            ),
        {
            params: t.Object({ id: t.String() })
        }
    )
    .post(
        "/documents/import",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        documentService.importDocument(session.user.id, ctx.body.sourcePath, ctx.body.content, ctx.body.spaceId)
                    )
                )
            ),
        {
            body: t.Object({
                sourcePath: t.String({ maxLength: 500 }),
                content: t.String({ maxLength: 200000 }),
                spaceId: t.Optional(t.String())
            })
        }
    )
    .post(
        "/documents/import-dir",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        Effect.forEach(ctx.body.files, file =>
                            documentService.importDocument(session.user.id, file.sourcePath, file.content, ctx.body.spaceId)
                        )
                    )
                )
            ),
        {
            body: t.Object({
                files: t.Array(
                    t.Object({
                        sourcePath: t.String({ maxLength: 500 }),
                        content: t.String({ maxLength: 200000 })
                    }),
                    { maxItems: 200 }
                ),
                spaceId: t.Optional(t.String())
            })
        }
    )
    .post(
        "/documents/:id/sync",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        documentService.syncDocument(ctx.params.id, ctx.body.content, session.user.id, ctx.body.spaceId)
                    )
                )
            ),
        {
            params: t.Object({ id: t.String() }),
            body: t.Object({
                content: t.String({ maxLength: 200000 }),
                spaceId: t.Optional(t.String())
            })
        }
    )
    .get(
        "/documents/:id/render",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => documentService.renderDocument(ctx.params.id, session.user.id)))
            ),
        {
            params: t.Object({ id: t.String() })
        }
    )
    .delete(
        "/documents/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => documentService.removeDocument(ctx.params.id, session.user.id)))
            ),
        {
            params: t.Object({ id: t.String() })
        }
    );
