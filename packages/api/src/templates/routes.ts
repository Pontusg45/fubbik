import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as templateService from "./service";

const MatchModeSchema = t.Union([t.Literal("exact"), t.Literal("prefix"), t.Literal("contains")]);

const MatchRuleHeadingSchema = t.Object({
    patterns: t.Array(t.String()),
    match: MatchModeSchema,
    level: t.Optional(t.Number()),
    required: t.Boolean()
});

const MatchRuleFrontmatterSchema = t.Object({
    key: t.String(),
    match: t.Union([t.Literal("exact"), t.Literal("oneOf"), t.Literal("exists")]),
    value: t.Optional(t.String()),
    values: t.Optional(t.Array(t.String()))
});

const MatchRulesSchema = t.Object({
    minScore: t.Number(),
    headings: t.Array(MatchRuleHeadingSchema),
    frontmatter: t.Array(MatchRuleFrontmatterSchema)
});

const ExtractionTargetSchema = t.Union([
    t.Literal("rationale"),
    t.Literal("alternatives"),
    t.Literal("consequences"),
    t.Literal("summary"),
    t.Literal("scope"),
    t.Literal("content")
]);

const FieldMappingSchema = t.Object({
    headings: t.Array(t.String()),
    match: MatchModeSchema,
    target: ExtractionTargetSchema
});

export const templateRoutes = new Elysia()
    .get("/templates", ctx =>
        Effect.runPromise(requireSession(ctx).pipe(Effect.flatMap(session => templateService.listTemplates(session.user.id))))
    )
    .post(
        "/templates",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => templateService.createTemplate(session.user.id, ctx.body)),
                    Effect.tap(() =>
                        Effect.sync(() => {
                            ctx.set.status = 201;
                        })
                    )
                )
            ),
        {
            body: t.Object({
                name: t.String({ maxLength: 100 }),
                description: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                type: t.String({ maxLength: 20 }),
                content: t.String({ maxLength: 50000 }),
                matchRules: t.Optional(t.Union([MatchRulesSchema, t.Null()])),
                fieldMappings: t.Optional(t.Union([t.Array(FieldMappingSchema), t.Null()])),
                priority: t.Optional(t.Number()),
                tags: t.Optional(t.Array(t.String({ maxLength: 50 }), { maxItems: 20 }))
            })
        }
    )
    .patch(
        "/templates/:id",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => templateService.updateTemplate(ctx.params.id, session.user.id, ctx.body))
                )
            ),
        {
            body: t.Object({
                name: t.Optional(t.String({ maxLength: 100 })),
                description: t.Optional(t.Union([t.String({ maxLength: 500 }), t.Null()])),
                type: t.Optional(t.String({ maxLength: 20 })),
                content: t.Optional(t.String({ maxLength: 50000 })),
                matchRules: t.Optional(t.Union([MatchRulesSchema, t.Null()])),
                fieldMappings: t.Optional(t.Union([t.Array(FieldMappingSchema), t.Null()])),
                priority: t.Optional(t.Number()),
                tags: t.Optional(t.Array(t.String({ maxLength: 50 }), { maxItems: 20 }))
            })
        }
    )
    .delete("/templates/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => templateService.deleteTemplate(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
