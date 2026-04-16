/**
 * Pure function that applies the /graph pre-filter to an in-memory chunk+connection set.
 *
 * Used in two places so results match exactly:
 *   - graph-view.tsx filteredGraph useMemo (what renders on screen)
 *   - graph-filter-dialog.tsx live preview (what we tell the user they'll see)
 *
 * Generic over chunk/connection shape so the caller's full row types flow through
 * (the useMemo downstream of this still needs createdAt, id, etc.).
 */

interface ChunkLike {
    id: string;
    type: string;
}
interface ConnectionLike {
    sourceId: string;
    targetId: string;
}

export interface PrefilterInput<C extends ChunkLike, E extends ConnectionLike> {
    chunks: C[];
    connections: E[];
    chunkTags?: Array<{ chunkId: string; tagName: string }>;
}

export interface PrefilterSpec {
    tags: string[];
    types: string[];
    focusChunkId: string | null;
    depth: number;
}

export interface PrefilterResult<C extends ChunkLike, E extends ConnectionLike> {
    chunks: C[];
    connections: E[];
}

export function applyPrefilter<C extends ChunkLike, E extends ConnectionLike>(
    input: PrefilterInput<C, E>,
    spec: PrefilterSpec
): PrefilterResult<C, E> {
    let chunks = input.chunks;
    let connections = input.connections;

    if (spec.types.length > 0) {
        const allowed = new Set(spec.types);
        chunks = chunks.filter(c => allowed.has(c.type));
    }

    if (spec.tags.length > 0 && input.chunkTags) {
        const allowedTags = new Set(spec.tags);
        const withTag = new Set<string>();
        for (const ct of input.chunkTags) if (allowedTags.has(ct.tagName)) withTag.add(ct.chunkId);
        chunks = chunks.filter(c => withTag.has(c.id));
    }

    if (spec.focusChunkId) {
        const focusId = spec.focusChunkId;
        const kept = new Set<string>([focusId]);
        let frontier = new Set<string>([focusId]);
        for (let hop = 0; hop < spec.depth; hop++) {
            const next = new Set<string>();
            for (const conn of connections) {
                if (frontier.has(conn.sourceId) && !kept.has(conn.targetId)) next.add(conn.targetId);
                if (frontier.has(conn.targetId) && !kept.has(conn.sourceId)) next.add(conn.sourceId);
            }
            for (const id of next) kept.add(id);
            frontier = next;
            if (frontier.size === 0) break;
        }
        chunks = chunks.filter(c => kept.has(c.id));
    }

    if (spec.tags.length > 0 || spec.types.length > 0 || spec.focusChunkId) {
        const visible = new Set(chunks.map(c => c.id));
        connections = connections.filter(c => visible.has(c.sourceId) && visible.has(c.targetId));
    }

    return { chunks, connections };
}
