import { chunk as chunkTable } from "@fubbik/db/schema/chunk";

import { computeHealthScore } from "../chunks/health-score";

type ChunkRow = typeof chunkTable.$inferSelect;

export interface ScoredChunk {
    id: string;
    title: string;
    content: string;
    type: string;
    rationale: string | null;
    tags: string[];
    score: number;
}

export function estimateTokens(text: string): number {
    return Math.ceil(text.length / 4);
}

export function scoreChunk(c: ChunkRow, connectionCount: number): number {
    const health = computeHealthScore({
        content: c.content,
        updatedAt: c.updatedAt,
        summary: c.summary,
        rationale: c.rationale,
        alternatives: c.alternatives,
        consequences: c.consequences,
        connectionCount,
        hasEmbedding: c.embedding != null,
        requirementCount: 0,
        allRequirementsPassing: false,
        referencedInSession: false
    });
    const healthPoints = health.total / 10; // 0-10 (already includes freshness)
    const typePoints = c.type === "document" ? 3 : c.type === "note" ? 1 : 2;
    const rationalePoints = c.rationale ? 2 : 0;
    const connectionPoints = Math.min(connectionCount * 2, 10);
    const reviewPoints = c.reviewStatus === "approved" ? 2 : c.reviewStatus === "reviewed" ? 1 : 0;
    // NO separate freshnessPoints — already in healthPoints from computeHealthScore
    return healthPoints + typePoints + rationalePoints + connectionPoints + reviewPoints;
}

export function budgetChunks<T extends ScoredChunk>(chunks: T[], maxTokens: number): T[] {
    const sorted = [...chunks].sort((a, b) => b.score - a.score);
    const selected: T[] = [];
    let usedTokens = estimateTokens("# Project Context\n\n");
    for (const chunk of sorted) {
        const chunkText = formatChunkText(chunk);
        const tokens = estimateTokens(chunkText);
        if (usedTokens + tokens > maxTokens) continue;
        selected.push(chunk);
        usedTokens += tokens;
    }
    return selected;
}

export function formatChunkText(chunk: ScoredChunk): string {
    const typeLabel =
        chunk.type === "document"
            ? "Architecture"
            : chunk.type === "convention"
              ? "Convention"
              : chunk.type === "note"
                ? "Note"
                : chunk.type.charAt(0).toUpperCase() + chunk.type.slice(1);
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
