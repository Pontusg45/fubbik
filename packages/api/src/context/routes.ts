import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { NotFoundError, ValidationError } from "../errors";
import { requireSession } from "../require-session";
import { formatStructured, formatStructuredMarkdown } from "./formatter";
import { enrichChunks, resolveForConcept, resolveForFiles, resolveForPlan } from "./resolvers";
import { budgetChunks } from "./utils";

const DEFAULT_MAX_TOKENS = 4000;

export const contextRoutes = new Elysia()
    // GET /context/for-plan?planId=X&maxTokens=N&format=structured-md
    .get(
        "/context/for-plan",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => {
                        const maxTokens = ctx.query.maxTokens
                            ? Number(ctx.query.maxTokens)
                            : DEFAULT_MAX_TOKENS;

                        if (!ctx.query.planId) {
                            return Effect.fail(new ValidationError({ message: "planId is required" }));
                        }

                        return resolveForPlan(ctx.query.planId).pipe(
                            Effect.flatMap(ids => enrichChunks(ids, session.user.id)),
                            Effect.map(chunks => {
                                const budgeted = budgetChunks(chunks, maxTokens);
                                const structured = formatStructured(budgeted);
                                const format = ctx.query.format ?? "structured-md";
                                if (format === "structured-json") {
                                    return { format: "structured-json" as const, ...structured };
                                }
                                return {
                                    format: "structured-md" as const,
                                    content: formatStructuredMarkdown(structured),
                                    totalChunks: structured.totalChunks,
                                };
                            }),
                        );
                    }),
                ),
            ),
        {
            query: t.Object({
                planId: t.String(),
                maxTokens: t.Optional(t.String()),
                format: t.Optional(t.Union([t.Literal("structured-md"), t.Literal("structured-json")])),
            }),
        },
    )
    // GET /context/about?q=auth&maxTokens=N&codebaseId=X&format=structured-md
    .get(
        "/context/about",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => {
                        const maxTokens = ctx.query.maxTokens
                            ? Number(ctx.query.maxTokens)
                            : DEFAULT_MAX_TOKENS;

                        if (!ctx.query.q) {
                            return Effect.fail(new ValidationError({ message: "q is required" }));
                        }

                        return resolveForConcept(
                            ctx.query.q,
                            session.user.id,
                            ctx.query.codebaseId,
                        ).pipe(
                            Effect.flatMap(ids => enrichChunks(ids, session.user.id)),
                            Effect.map(chunks => {
                                const budgeted = budgetChunks(chunks, maxTokens);
                                const structured = formatStructured(budgeted);
                                const format = ctx.query.format ?? "structured-md";
                                if (format === "structured-json") {
                                    return { format: "structured-json" as const, ...structured };
                                }
                                return {
                                    format: "structured-md" as const,
                                    content: formatStructuredMarkdown(structured),
                                    totalChunks: structured.totalChunks,
                                };
                            }),
                        );
                    }),
                ),
            ),
        {
            query: t.Object({
                q: t.String(),
                maxTokens: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
                format: t.Optional(t.Union([t.Literal("structured-md"), t.Literal("structured-json")])),
            }),
        },
    )
    // GET /context/for-files?paths=a.ts,b.ts&maxTokens=N&codebaseId=X&format=structured-md
    .get(
        "/context/for-files",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => {
                        const maxTokens = ctx.query.maxTokens
                            ? Number(ctx.query.maxTokens)
                            : DEFAULT_MAX_TOKENS;

                        if (!ctx.query.paths) {
                            return Effect.fail(new ValidationError({ message: "paths is required" }));
                        }

                        const paths = ctx.query.paths.split(",").map(p => p.trim()).filter(Boolean);
                        if (paths.length === 0) {
                            return Effect.fail(new ValidationError({ message: "paths must contain at least one path" }));
                        }

                        return resolveForFiles(paths, session.user.id, ctx.query.codebaseId).pipe(
                            Effect.flatMap(ids => enrichChunks(ids, session.user.id)),
                            Effect.map(chunks => {
                                const budgeted = budgetChunks(chunks, maxTokens);
                                const structured = formatStructured(budgeted);
                                const format = ctx.query.format ?? "structured-md";
                                if (format === "structured-json") {
                                    return { format: "structured-json" as const, ...structured };
                                }
                                return {
                                    format: "structured-md" as const,
                                    content: formatStructuredMarkdown(structured),
                                    totalChunks: structured.totalChunks,
                                };
                            }),
                        );
                    }),
                ),
            ),
        {
            query: t.Object({
                paths: t.String(),
                maxTokens: t.Optional(t.String()),
                codebaseId: t.Optional(t.String()),
                format: t.Optional(t.Union([t.Literal("structured-md"), t.Literal("structured-json")])),
            }),
        },
    );
