import { getAllChunksMeta, getAllConnectionsForUser } from "@fubbik/db/repository";
import { Effect } from "effect";

const DEFAULT_MAX_NODES = 100;

interface DiagramOptions {
    codebaseId?: string;
    workspaceId?: string;
    maxNodes?: number;
    direction?: "LR" | "TB";
}

export function generateDiagram(userId: string, codebaseIdOrOpts: string | DiagramOptions) {
    const opts: DiagramOptions =
        typeof codebaseIdOrOpts === "string" ? { codebaseId: codebaseIdOrOpts } : codebaseIdOrOpts;
    const maxNodes = opts.maxNodes ?? DEFAULT_MAX_NODES;
    const direction = opts.direction ?? "LR";

    return Effect.all({
        chunks: getAllChunksMeta(userId, opts.codebaseId, opts.workspaceId),
        connections: getAllConnectionsForUser(userId)
    }).pipe(
        Effect.map(({ chunks, connections }) => {
            const truncated = chunks.length > maxNodes;
            const keptChunks = truncated ? chunks.slice(0, maxNodes) : chunks;
            const keptIds = new Set(keptChunks.map(c => c.id));
            const keptConnections = connections.filter(c => keptIds.has(c.sourceId) && keptIds.has(c.targetId));

            const lines: string[] = [`flowchart ${direction}`];
            for (const chunk of keptChunks) {
                lines.push(`    ${sanitizeId(chunk.id)}["${escapeLabel(chunk.title)}"]`);
            }
            for (const conn of keptConnections) {
                lines.push(
                    `    ${sanitizeId(conn.sourceId)} -->|${escapeLabel(conn.relation)}| ${sanitizeId(conn.targetId)}`
                );
            }

            return {
                diagram: lines.join("\n"),
                nodeCount: keptChunks.length,
                edgeCount: keptConnections.length,
                truncated
            };
        })
    );
}

/** Convert a UUID-style id into a valid Mermaid node identifier */
function sanitizeId(id: string): string {
    return "n" + id.replace(/[^a-zA-Z0-9]/g, "");
}

function escapeLabel(raw: string): string {
    return raw.replace(/"/g, "'").replace(/[[\]]/g, "").replace(/\|/g, "／");
}
