export interface NeighborhoodInput {
    focusChunkId: string;
    chunks: Array<{ id: string }>;
    connections: Array<{ sourceId: string; targetId: string; relation: string }>;
}

export interface NeighborhoodResult {
    positions: Record<string, { x: number; y: number }>;
    hops: Map<string, number>;
    visibleConnections: Array<{ sourceId: string; targetId: string; relation: string }>;
}

const INNER_RING_RADIUS = 250;
const OUTER_RING_RADIUS = 450;
const MAX_HOPS = 2;

export function layoutNeighborhood(input: NeighborhoodInput): NeighborhoodResult {
    const { focusChunkId, chunks, connections } = input;

    const adjacency = new Map<string, Array<{ neighbor: string; relation: string }>>();
    for (const conn of connections) {
        const fromList = adjacency.get(conn.sourceId) ?? [];
        fromList.push({ neighbor: conn.targetId, relation: conn.relation });
        adjacency.set(conn.sourceId, fromList);
        const toList = adjacency.get(conn.targetId) ?? [];
        toList.push({ neighbor: conn.sourceId, relation: conn.relation });
        adjacency.set(conn.targetId, toList);
    }

    const chunkIdSet = new Set(chunks.map(c => c.id));
    const hops = new Map<string, number>();
    hops.set(focusChunkId, 0);
    const queue = [focusChunkId];
    let qi = 0;

    while (qi < queue.length) {
        const current = queue[qi++]!;
        const currentHop = hops.get(current)!;
        if (currentHop >= MAX_HOPS) continue;

        for (const { neighbor } of adjacency.get(current) ?? []) {
            if (!hops.has(neighbor) && chunkIdSet.has(neighbor)) {
                hops.set(neighbor, currentHop + 1);
                queue.push(neighbor);
            }
        }
    }

    const positions: Record<string, { x: number; y: number }> = {};
    positions[focusChunkId] = { x: 0, y: 0 };

    const byHop = new Map<number, string[]>();
    for (const [id, hop] of hops) {
        if (id === focusChunkId) continue;
        const list = byHop.get(hop) ?? [];
        list.push(id);
        byHop.set(hop, list);
    }

    for (const [hop, ids] of byHop) {
        const radius = hop === 1 ? INNER_RING_RADIUS : OUTER_RING_RADIUS;
        for (let i = 0; i < ids.length; i++) {
            const angle = (2 * Math.PI * i) / ids.length - Math.PI / 2;
            positions[ids[i]!] = {
                x: Math.round(Math.cos(angle) * radius),
                y: Math.round(Math.sin(angle) * radius)
            };
        }
    }

    const visibleIds = new Set(Object.keys(positions));
    const visibleConnections = connections.filter(c => visibleIds.has(c.sourceId) && visibleIds.has(c.targetId));

    return { positions, hops, visibleConnections };
}
