import {
    listChunks as listChunksRepo,
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { enrichChunks, resolveForFiles } from "../context/resolvers";
import { formatStructured, formatStructuredMarkdown } from "../context/formatter";
import { budgetChunks, estimateTokens, formatChunkText } from "../context/utils";

interface ContextExportQuery {
    codebaseId?: string;
    maxTokens?: number;
    format?: "markdown" | "json";
    forPath?: string;
}

export function exportContext(userId: string, query: ContextExportQuery) {
    const maxTokens = query.maxTokens ?? 4000;
    const format = query.format ?? "markdown";

    const fetchApproved = listChunksRepo({
        userId,
        reviewStatus: "approved",
        codebaseId: query.codebaseId,
        limit: 500,
        offset: 0,
    });

    const fetchOthers = listChunksRepo({
        userId,
        codebaseId: query.codebaseId,
        limit: 500,
        offset: 0,
    });

    return Effect.all({ approved: fetchApproved, all: fetchOthers }).pipe(
        Effect.flatMap(({ approved, all }) => {
            const approvedIds = new Set(approved.chunks.map(c => c.id));
            const otherChunks = all.chunks.filter(c => !approvedIds.has(c.id));
            const combined = [...approved.chunks, ...otherChunks];
            const chunkIds = combined.map(c => c.id);

            return enrichChunks(chunkIds, userId).pipe(
                Effect.flatMap(enriched =>
                    Effect.gen(function* () {
                        if (query.forPath) {
                            const fileIds = yield* resolveForFiles(
                                [query.forPath],
                                userId,
                                query.codebaseId,
                            );
                            const fileIdSet = new Set(fileIds);
                            for (const item of enriched) {
                                if (fileIdSet.has(item.id)) {
                                    item.score += 15;
                                }
                            }
                        }

                        const budgeted = budgetChunks(enriched, maxTokens);

                        if (format === "json") {
                            return {
                                format: "json" as const,
                                tokens: budgeted.reduce(
                                    (sum, c) => sum + estimateTokens(formatChunkText(c)),
                                    estimateTokens("# Project Context\n\n"),
                                ),
                                chunks: budgeted.map(c => ({
                                    title: c.title,
                                    content: c.content,
                                    type: c.type,
                                    tags: c.tags,
                                })),
                                content: undefined as string | undefined,
                            };
                        }

                        const structured = formatStructured(budgeted);
                        const content = formatStructuredMarkdown(structured);

                        return {
                            format: "markdown" as const,
                            tokens: estimateTokens(content),
                            chunks: undefined as
                                | { title: string; content: string; type: string; tags: string[] }[]
                                | undefined,
                            content,
                        };
                    }),
                ),
            );
        }),
    );
}
