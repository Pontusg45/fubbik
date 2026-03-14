import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as aiService from "./service";
import { structureRequirement } from "./structure-requirement";

export const aiRoutes = new Elysia()
    .post(
        "/ai/summarize",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => aiService.summarizeChunkById(ctx.body.chunkId, session.user.id)))
            ),
        { body: t.Object({ chunkId: t.String() }) }
    )
    .post(
        "/ai/suggest-connections",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(Effect.flatMap(session => aiService.suggestConnectionsForChunk(ctx.body.chunkId, session.user.id)))
            ),
        { body: t.Object({ chunkId: t.String() }) }
    )
    .post(
        "/ai/generate",
        ctx => Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(() => aiService.generateChunk(ctx.body.prompt)))),
        { body: t.Object({ prompt: t.String() }) }
    )
    .post(
        "/ai/structure-requirement",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() => structureRequirement(ctx.body.description, ctx.body.codebaseId))
                )
            ),
        {
            body: t.Object({
                description: t.String({ maxLength: 5000 }),
                codebaseId: t.Optional(t.String())
            })
        }
    );
