import { getAllChunksMeta, getAllConnectionsForUser } from "@fubbik/db/repository";
import { Effect } from "effect";

export function generateDiagram(userId: string, codebaseId: string) {
    return Effect.all({
        chunks: getAllChunksMeta(userId, codebaseId),
        connections: getAllConnectionsForUser(userId)
    }).pipe(
        Effect.map(({ chunks, connections }) => {
            const chunkMap = new Map(chunks.map(c => [c.id, c.title]));

            // Filter connections to only those where both source and target are in the codebase chunks
            const relevantConnections = connections.filter(
                c => chunkMap.has(c.sourceId) && chunkMap.has(c.targetId)
            );

            const lines: string[] = ["graph TD"];

            // Create node declarations with sanitised labels
            for (const chunk of chunks) {
                const safeTitle = chunk.title.replace(/"/g, "'").replace(/[[\]]/g, "");
                lines.push(`    ${sanitizeId(chunk.id)}["${safeTitle}"]`);
            }

            // Create edges
            for (const conn of relevantConnections) {
                const safeRelation = conn.relation.replace(/"/g, "'");
                lines.push(`    ${sanitizeId(conn.sourceId)} -->|${safeRelation}| ${sanitizeId(conn.targetId)}`);
            }

            return { diagram: lines.join("\n") };
        })
    );
}

/** Convert a UUID-style id into a valid Mermaid node identifier */
function sanitizeId(id: string): string {
    return "n" + id.replace(/-/g, "");
}
