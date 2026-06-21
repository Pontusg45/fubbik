import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { formatBehaviorsMarkdown, formatStructured, formatStructuredMarkdown } from "../context/formatter";
import type { GoverningBehavior } from "../context/formatter";
import { enrichChunks, resolveForFiles } from "../context/resolvers";
import { budgetChunks } from "../context/utils";
import { getBehaviorsForCodePath } from "../matrices/service";
import { requireSession } from "../require-session";
import { getContextForFile } from "./service";

const DEFAULT_MAX_TOKENS = 4000;

export const contextForFileRoutes = new Elysia().get(
    "/context/for-file",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap((session): Effect.Effect<Record<string, unknown>, unknown> => {
                    const format = ctx.query.format ?? "structured-md";

                    // Legacy JSON format for backwards compatibility
                    if (format === "json-legacy") {
                        return getContextForFile(
                            session.user.id,
                            ctx.query.path,
                            ctx.query.spaceId,
                            ctx.query.deps ? ctx.query.deps.split(",").filter(Boolean) : undefined
                        ).pipe(Effect.map(result => ({ ...result })));
                    }

                    const maxTokens = ctx.query.maxTokens ? Number(ctx.query.maxTokens) : DEFAULT_MAX_TOKENS;

                    // Governing behaviors are optional context — a failure here
                    // (e.g. matrix tables empty) must never break file context.
                    const behaviors = getBehaviorsForCodePath(session.user.id, ctx.query.path).pipe(
                        Effect.catchAll((): Effect.Effect<GoverningBehavior[]> => Effect.succeed([]))
                    );

                    return Effect.all({
                        chunks: resolveForFiles([ctx.query.path], session.user.id, ctx.query.spaceId).pipe(
                            Effect.flatMap(ids => enrichChunks(ids, session.user.id))
                        ),
                        behaviors
                    }).pipe(
                        Effect.map(({ chunks, behaviors: governing }) => {
                            const budgeted = budgetChunks(chunks, maxTokens);
                            const structured = formatStructured(budgeted);
                            if (format === "structured-json") {
                                return { format: "structured-json" as const, ...structured, behaviors: governing };
                            }
                            const behaviorsMd = formatBehaviorsMarkdown(governing);
                            const content = behaviorsMd
                                ? `${formatStructuredMarkdown(structured)}\n\n${behaviorsMd}`
                                : formatStructuredMarkdown(structured);
                            return {
                                format: "structured-md" as const,
                                content,
                                totalChunks: structured.totalChunks
                            };
                        })
                    );
                })
            )
        ),
    {
        query: t.Object({
            path: t.String(),
            spaceId: t.Optional(t.String()),
            deps: t.Optional(t.String()),
            format: t.Optional(t.Union([t.Literal("structured-md"), t.Literal("structured-json"), t.Literal("json-legacy")])),
            maxTokens: t.Optional(t.String())
        })
    }
);
