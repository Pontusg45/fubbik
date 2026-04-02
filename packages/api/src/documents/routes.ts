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
                    Effect.flatMap(session =>
                        documentService.listDocuments(session.user.id, ctx.query.codebaseId)
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .get(
        "/documents/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => documentService.getDocument(ctx.params.id))
                )
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
                        documentService.importDocument(
                            session.user.id,
                            ctx.body.sourcePath,
                            ctx.body.content,
                            ctx.body.codebaseId
                        )
                    )
                )
            ),
        {
            body: t.Object({
                sourcePath: t.String({ maxLength: 500 }),
                content: t.String({ maxLength: 200000 }),
                codebaseId: t.Optional(t.String())
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
                            documentService.importDocument(
                                session.user.id,
                                file.sourcePath,
                                file.content,
                                ctx.body.codebaseId
                            )
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
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .post(
        "/documents/:id/sync",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        documentService.syncDocument(
                            ctx.params.id,
                            ctx.body.content,
                            session.user.id,
                            ctx.body.codebaseId
                        )
                    )
                )
            ),
        {
            params: t.Object({ id: t.String() }),
            body: t.Object({
                content: t.String({ maxLength: 200000 }),
                codebaseId: t.Optional(t.String())
            })
        }
    )
    .get(
        "/documents/:id/render",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => documentService.renderDocument(ctx.params.id))
                )
            ),
        {
            params: t.Object({ id: t.String() })
        }
    )
    .delete(
        "/documents/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => documentService.removeDocument(ctx.params.id))
                )
            ),
        {
            params: t.Object({ id: t.String() })
        }
    );
