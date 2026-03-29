import type { Edge } from "@xyflow/react";

/**
 * BFS shortest path between two nodes in an undirected graph.
 * Returns the ordered list of node IDs, or null if no path exists.
 */
export function findShortestPath(startId: string, endId: string, edges: Edge[]): string[] | null {
    const adjacency = new Map<string, string[]>();
    for (const edge of edges) {
        if (!adjacency.has(edge.source)) adjacency.set(edge.source, []);
        if (!adjacency.has(edge.target)) adjacency.set(edge.target, []);
        adjacency.get(edge.source)!.push(edge.target);
        adjacency.get(edge.target)!.push(edge.source);
    }
    const visited = new Set<string>([startId]);
    const queue: { nodeId: string; path: string[] }[] = [{ nodeId: startId, path: [startId] }];
    while (queue.length > 0) {
        const { nodeId, path } = queue.shift()!;
        if (nodeId === endId) return path;
        for (const neighbor of adjacency.get(nodeId) ?? []) {
            if (!visited.has(neighbor)) {
                visited.add(neighbor);
                queue.push({ nodeId: neighbor, path: [...path, neighbor] });
            }
        }
    }
    return null;
}

/**
 * Returns the ID of the node with the most connections (edges), or null if empty.
 */
export function getMostConnected(nodes: { id: string }[], edges: { source: string; target: string }[]): string | null {
    const counts = new Map<string, number>();
    for (const e of edges) {
        counts.set(e.source, (counts.get(e.source) ?? 0) + 1);
        counts.set(e.target, (counts.get(e.target) ?? 0) + 1);
    }
    let maxId: string | null = null;
    let maxCount = 0;
    for (const [id, count] of counts) {
        if (count > maxCount && nodes.some(n => n.id === id)) {
            maxCount = count;
            maxId = id;
        }
    }
    return maxId;
}
