import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import { formatStructured, formatStructuredMarkdown } from "../context/formatter";
import { enrichChunks, resolveForFiles } from "../context/resolvers";
import { budgetChunks } from "../context/utils";
import { getContextForFile } from "./service";

const DEFAULT_MAX_TOKENS = 4000;

export const contextForFileRoutes = new Elysia().get(
    "/context/for-file",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => {
                    const format = ctx.query.format ?? "structured-md";

                    // Legacy JSON format for backwards compatibility
                    if (format === "json-legacy") {
                        return getContextForFile(
                            session.user.id,
                            ctx.query.path,
                            ctx.query.codebaseId,
                            ctx.query.deps ? ctx.query.deps.split(",").filter(Boolean) : undefined
                        ).pipe(Effect.map(result => result as Record<string, unknown>));
                    }

                    const maxTokens = ctx.query.maxTokens
                        ? Number(ctx.query.maxTokens)
                        : DEFAULT_MAX_TOKENS;

                    return resolveForFiles(
                        [ctx.query.path],
                        session.user.id,
                        ctx.query.codebaseId,
                    ).pipe(
                        Effect.flatMap(ids => enrichChunks(ids, session.user.id)),
                        Effect.map(chunks => {
                            const budgeted = budgetChunks(chunks, maxTokens);
                            const structured = formatStructured(budgeted);
                            if (format === "structured-json") {
                                return { format: "structured-json" as const, ...structured } as Record<string, unknown>;
                            }
                            return {
                                format: "structured-md" as const,
                                content: formatStructuredMarkdown(structured),
                                totalChunks: structured.totalChunks,
                            } as Record<string, unknown>;
                        }),
                    );
                }),
            ),
        ),
    {
        query: t.Object({
            path: t.String(),
            codebaseId: t.Optional(t.String()),
            deps: t.Optional(t.String()),
            format: t.Optional(
                t.Union([
                    t.Literal("structured-md"),
                    t.Literal("structured-json"),
                    t.Literal("json-legacy"),
                ]),
            ),
            maxTokens: t.Optional(t.String()),
        }),
    },
);
