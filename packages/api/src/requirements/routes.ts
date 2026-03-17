import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as requirementService from "./service";

const StepSchema = t.Object({
    keyword: t.Union([
        t.Literal("given"),
        t.Literal("when"),
        t.Literal("then"),
        t.Literal("and"),
        t.Literal("but")
    ]),
    text: t.String({ maxLength: 1000 }),
    params: t.Optional(t.Record(t.String(), t.String()))
});

const StatusSchema = t.Union([
    t.Literal("passing"),
    t.Literal("failing"),
    t.Literal("untested")
]);

const PrioritySchema = t.Optional(
    t.Union([
        t.Literal("must"),
        t.Literal("should"),
        t.Literal("could"),
        t.Literal("wont")
    ])
);

const FormatSchema = t.Union([
    t.Literal("gherkin"),
    t.Literal("vitest"),
    t.Literal("markdown")
]);

export const requirementRoutes = new Elysia()
    // 1. Stats
    .get(
        "/requirements/stats",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* requirementService.getStats(session.user.id, ctx.query.codebaseId);
                })
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String())
            })
        }
    )
    // 2. Bulk operations
    .patch(
        "/requirements/bulk",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* requirementService.bulkAction(session.user.id, ctx.body);
                })
            ),
        {
            body: t.Object({
                ids: t.Array(t.String(), { minItems: 1, maxItems: 100 }),
                action: t.Union([
                    t.Literal("set_status"),
                    t.Literal("set_use_case"),
                    t.Literal("delete")
                ]),
                status: t.Optional(StatusSchema),
                useCaseId: t.Optional(t.Union([t.String(), t.Null()]))
            })
        }
    )
    // 3. Export all
    .get(
        "/requirements/export",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* requirementService.exportAll(session.user.id, {
                        codebaseId: ctx.query.codebaseId,
                        format: ctx.query.format
                    });
                })
            ),
        {
            query: t.Object({
                format: FormatSchema,
                codebaseId: t.Optional(t.String())
            })
        }
    )
    // 4. List
    .get(
        "/requirements",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* requirementService.listRequirements(session.user.id, ctx.query);
                })
            ),
        {
            query: t.Object({
                codebaseId: t.Optional(t.String()),
                useCaseId: t.Optional(t.String()),
                search: t.Optional(t.String()),
                status: t.Optional(t.String()),
                priority: t.Optional(t.String()),
                origin: t.Optional(t.Union([t.Literal("human"), t.Literal("ai")])),
                reviewStatus: t.Optional(t.Union([t.Literal("draft"), t.Literal("reviewed"), t.Literal("approved")])),
                limit: t.Optional(t.String()),
                offset: t.Optional(t.String())
            })
        }
    )
    // 5. Create
    .post(
        "/requirements",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    const result = yield* requirementService.createRequirement(session.user.id, ctx.body);
                    ctx.set.status = 201;
                    return result;
                })
            ),
        {
            body: t.Object({
                title: t.String({ maxLength: 200 }),
                description: t.Optional(t.String({ maxLength: 5000 })),
                steps: t.Array(StepSchema, { minItems: 1 }),
                priority: PrioritySchema,
                codebaseId: t.Optional(t.String()),
                useCaseId: t.Optional(t.String()),
                origin: t.Optional(t.Union([t.Literal("human"), t.Literal("ai")]))
            })
        }
    )
    // 6. Get by ID
    .get("/requirements/:id", ctx =>
        Effect.runPromise(
            Effect.gen(function* () {
                const session = yield* requireSession(ctx);
                return yield* requirementService.getRequirement(ctx.params.id, session.user.id);
            })
        )
    )
    // 7. Update
    .patch(
        "/requirements/:id",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* requirementService.updateRequirement(ctx.params.id, session.user.id, ctx.body);
                })
            ),
        {
            body: t.Object({
                title: t.Optional(t.String({ maxLength: 200 })),
                description: t.Optional(t.Union([t.String({ maxLength: 5000 }), t.Null()])),
                steps: t.Optional(t.Array(StepSchema, { minItems: 1 })),
                priority: t.Optional(t.Union([
                    t.Literal("must"),
                    t.Literal("should"),
                    t.Literal("could"),
                    t.Literal("wont"),
                    t.Null()
                ])),
                codebaseId: t.Optional(t.Union([t.String(), t.Null()])),
                useCaseId: t.Optional(t.Union([t.String(), t.Null()])),
                origin: t.Optional(t.Union([t.Literal("human"), t.Literal("ai")])),
                reviewStatus: t.Optional(t.Union([t.Literal("draft"), t.Literal("reviewed"), t.Literal("approved")]))
            })
        }
    )
    // 8. Delete
    .delete("/requirements/:id", ctx =>
        Effect.runPromise(
            Effect.gen(function* () {
                const session = yield* requireSession(ctx);
                yield* requirementService.deleteRequirement(ctx.params.id, session.user.id);
                return { message: "Deleted" };
            })
        )
    )
    // 9. Update status
    .patch(
        "/requirements/:id/status",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* requirementService.updateStatus(ctx.params.id, session.user.id, ctx.body.status);
                })
            ),
        {
            body: t.Object({
                status: StatusSchema
            })
        }
    )
    // 10. Set chunks
    .put(
        "/requirements/:id/chunks",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* requirementService.setChunks(ctx.params.id, session.user.id, ctx.body.chunkIds);
                })
            ),
        {
            body: t.Object({
                chunkIds: t.Array(t.String(), { maxItems: 100 })
            })
        }
    )
    // 11. Export single
    .get(
        "/requirements/:id/export",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* requirementService.exportRequirement(ctx.params.id, session.user.id, ctx.query.format);
                })
            ),
        {
            query: t.Object({
                format: FormatSchema
            })
        }
    );
