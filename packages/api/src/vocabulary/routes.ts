import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as vocabularyService from "./service";

const CategorySchema = t.Union([
    t.Literal("actor"),
    t.Literal("action"),
    t.Literal("target"),
    t.Literal("outcome"),
    t.Literal("state"),
    t.Literal("modifier")
]);

const EntrySchema = t.Object({
    word: t.String({ maxLength: 100 }),
    category: CategorySchema,
    expects: t.Optional(t.Array(t.String()))
});

export const vocabularyRoutes = new Elysia()
    // 1. List vocabulary
    .get(
        "/vocabulary",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* vocabularyService.listVocabulary(session.user.id, ctx.query.codebaseId);
                })
            ),
        {
            query: t.Object({
                codebaseId: t.String()
            })
        }
    )
    // 2. Suggest vocabulary from chunks
    .post(
        "/vocabulary/suggest",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* vocabularyService.suggestFromChunks(session.user.id, ctx.body.codebaseId);
                })
            ),
        {
            body: t.Object({
                codebaseId: t.String()
            })
        }
    )
    // 3. Bulk create entries
    .post(
        "/vocabulary/bulk",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    const result = yield* vocabularyService.createEntries(session.user.id, ctx.body);
                    ctx.set.status = 201;
                    return result;
                })
            ),
        {
            body: t.Object({
                entries: t.Array(EntrySchema),
                codebaseId: t.String()
            })
        }
    )
    // 4. Parse step text
    .post(
        "/vocabulary/parse",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* vocabularyService.parseStep(session.user.id, ctx.body);
                })
            ),
        {
            body: t.Object({
                text: t.String({ maxLength: 1000 }),
                codebaseId: t.String()
            })
        }
    )
    // 5. Create single entry
    .post(
        "/vocabulary",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    const result = yield* vocabularyService.createEntry(session.user.id, ctx.body);
                    ctx.set.status = 201;
                    return result;
                })
            ),
        {
            body: t.Object({
                word: t.String({ maxLength: 100 }),
                category: CategorySchema,
                expects: t.Optional(t.Array(t.String())),
                codebaseId: t.String()
            })
        }
    )
    // 6. Update entry
    .patch(
        "/vocabulary/:id",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* vocabularyService.updateEntry(ctx.params.id, session.user.id, ctx.body);
                })
            ),
        {
            body: t.Object({
                word: t.Optional(t.String({ maxLength: 100 })),
                category: t.Optional(CategorySchema),
                expects: t.Optional(t.Array(t.String()))
            })
        }
    )
    // 7. Delete entry
    .delete("/vocabulary/:id", ctx =>
        Effect.runPromise(
            Effect.gen(function* () {
                const session = yield* requireSession(ctx);
                yield* vocabularyService.deleteEntry(ctx.params.id, session.user.id);
                return { message: "Deleted" };
            })
        )
    );
