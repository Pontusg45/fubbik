import {
    getChunkConnections,
    getTagsForChunks,
    listChunks as listChunksRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

interface ContextExportQuery {
    codebaseId?: string;
    maxTokens?: number;
    format?: "markdown" | "json";
}

interface ScoredChunk {
    id: string;
    title: string;
    content: string;
    type: string;
    rationale: string | null;
    tags: string[];
    score: number;
}

function estimateTokens(text: string): number {
    return Math.ceil(text.length / 4);
}

export function exportContext(userId: string, query: ContextExportQuery) {
    const maxTokens = query.maxTokens ?? 4000;
    const format = query.format ?? "markdown";

    // Fetch approved chunks first, then others
    const fetchApproved = listChunksRepo({
        userId,
        reviewStatus: "approved",
        codebaseId: query.codebaseId,
        limit: 500,
        offset: 0
    });

    const fetchOthers = listChunksRepo({
        userId,
        codebaseId: query.codebaseId,
        limit: 500,
        offset: 0
    });

    return Effect.all({ approved: fetchApproved, all: fetchOthers }).pipe(
        Effect.flatMap(({ approved, all }) => {
            // Deduplicate: approved first, then non-approved from the "all" set
            const approvedIds = new Set(approved.chunks.map(c => c.id));
            const otherChunks = all.chunks.filter(c => !approvedIds.has(c.id));
            const combined = [...approved.chunks, ...otherChunks];
            const chunkIds = combined.map(c => c.id);

            return Effect.all({
                chunks: Effect.succeed(combined),
                tags: getTagsForChunks(chunkIds),
                connections: Effect.all(
                    chunkIds.map(id => getChunkConnections(id).pipe(Effect.map(conns => ({ chunkId: id, count: conns.length })))),
                    { concurrency: 10 }
                )
            });
        }),
        Effect.map(({ chunks, tags, connections }) => {
            // Build tag map
            const tagMap = new Map<string, string[]>();
            for (const t of tags) {
                const existing = tagMap.get(t.chunkId) ?? [];
                existing.push(t.tagName);
                tagMap.set(t.chunkId, existing);
            }

            // Build connection count map
            const connMap = new Map<string, number>();
            for (const c of connections) {
                connMap.set(c.chunkId, c.count);
            }

            // Score chunks
            const scored: ScoredChunk[] = chunks.map(c => {
                const connectionCount = connMap.get(c.id) ?? 0;
                const typeScore = c.type === "document" ? 3 : c.type === "note" ? 1 : 2;
                const hasRationale = c.rationale ? 2 : 0;
                const score = connectionCount * 2 + typeScore + hasRationale;

                return {
                    id: c.id,
                    title: c.title,
                    content: c.content,
                    type: c.type,
                    rationale: c.rationale,
                    tags: tagMap.get(c.id) ?? [],
                    score
                };
            });

            // Sort by score descending
            scored.sort((a, b) => b.score - a.score);

            // Greedily fill budget
            const selected: ScoredChunk[] = [];
            let usedTokens = 0;
            const headerTokens = estimateTokens("# Project Context\n\n");

            usedTokens += headerTokens;
            for (const chunk of scored) {
                const chunkText = formatChunkText(chunk);
                const tokens = estimateTokens(chunkText);
                if (usedTokens + tokens > maxTokens) continue;
                selected.push(chunk);
                usedTokens += tokens;
            }

            if (format === "json") {
                return {
                    format: "json" as const,
                    tokens: usedTokens,
                    chunks: selected.map(c => ({
                        title: c.title,
                        content: c.content,
                        type: c.type,
                        tags: c.tags
                    })),
                    content: undefined as string | undefined
                };
            }

            // Markdown format
            const sections = selected.map(c => formatChunkText(c));
            const markdown = `# Project Context\n\n${sections.join("\n\n")}`;

            return {
                format: "markdown" as const,
                tokens: usedTokens,
                chunks: undefined as { title: string; content: string; type: string; tags: string[] }[] | undefined,
                content: markdown
            };
        })
    );
}

function formatChunkText(chunk: ScoredChunk): string {
    const typeLabel = chunk.type === "document" ? "Architecture" : chunk.type === "convention" ? "Convention" : chunk.type === "note" ? "Note" : chunk.type.charAt(0).toUpperCase() + chunk.type.slice(1);
    const header = `## ${typeLabel}: ${chunk.title}`;
    const parts = [header];
    if (chunk.content) {
        parts.push(chunk.content);
    }
    if (chunk.rationale) {
        parts.push(`**Rationale:** ${chunk.rationale}`);
    }
    return parts.join("\n");
}
