interface NodeInput {
    id: string;
    type: string;
}
interface EdgeInput {
    source: string;
    target: string;
    relation: string;
}

export type LayoutAlgorithm = "force" | "hierarchical" | "radial";

export function hierarchicalLayout(
    nodes: NodeInput[],
    edges: EdgeInput[]
): Record<string, { x: number; y: number }> {
    const children = new Map<string, string[]>();
    const hasParent = new Set<string>();

    for (const edge of edges) {
        if (edge.relation === "part_of" || edge.relation === "depends_on") {
            if (!children.has(edge.target)) children.set(edge.target, []);
            children.get(edge.target)!.push(edge.source);
            hasParent.add(edge.source);
        }
    }

    const roots = nodes.filter((n) => !hasParent.has(n.id)).map((n) => n.id);
    const positions: Record<string, { x: number; y: number }> = {};
    const LEVEL_HEIGHT = 150;
    const NODE_SPACING = 200;
    let currentX = 0;

    function layoutTree(nodeId: string, depth: number): number {
        const kids = children.get(nodeId) ?? [];
        if (kids.length === 0) {
            positions[nodeId] = { x: currentX, y: depth * LEVEL_HEIGHT };
            currentX += NODE_SPACING;
            return currentX - NODE_SPACING;
        }
        const childXs = kids.map((kid) => layoutTree(kid, depth + 1));
        const x = (childXs[0]! + childXs[childXs.length - 1]!) / 2;
        positions[nodeId] = { x, y: depth * LEVEL_HEIGHT };
        return x;
    }

    for (const root of roots) {
        layoutTree(root, 0);
        currentX += NODE_SPACING;
    }

    const placed = new Set(Object.keys(positions));
    let orphanX = 0;
    const maxY =
        Math.max(...Object.values(positions).map((p) => p.y), 0) +
        LEVEL_HEIGHT * 2;
    for (const node of nodes) {
        if (!placed.has(node.id)) {
            positions[node.id] = { x: orphanX, y: maxY };
            orphanX += NODE_SPACING;
        }
    }

    return positions;
}

export function radialLayout(
    nodes: NodeInput[],
    edges: EdgeInput[],
    centerId: string
): Record<string, { x: number; y: number }> {
    const adjacency = new Map<string, string[]>();
    for (const edge of edges) {
        if (!adjacency.has(edge.source)) adjacency.set(edge.source, []);
        if (!adjacency.has(edge.target)) adjacency.set(edge.target, []);
        adjacency.get(edge.source)!.push(edge.target);
        adjacency.get(edge.target)!.push(edge.source);
    }

    const positions: Record<string, { x: number; y: number }> = {};
    const visited = new Set<string>([centerId]);
    const levels: string[][] = [[centerId]];

    let current = [centerId];
    while (current.length > 0) {
        const next: string[] = [];
        for (const nodeId of current) {
            for (const neighbor of adjacency.get(nodeId) ?? []) {
                if (!visited.has(neighbor)) {
                    visited.add(neighbor);
                    next.push(neighbor);
                }
            }
        }
        if (next.length > 0) levels.push(next);
        current = next;
    }

    positions[centerId] = { x: 0, y: 0 };
    const RING_SPACING = 200;
    for (let i = 1; i < levels.length; i++) {
        const ring = levels[i]!;
        const radius = i * RING_SPACING;
        for (let j = 0; j < ring.length; j++) {
            const angle = (2 * Math.PI * j) / ring.length;
            positions[ring[j]!] = {
                x: Math.cos(angle) * radius,
                y: Math.sin(angle) * radius,
            };
        }
    }

    const unvisited = nodes.filter((n) => !visited.has(n.id));
    if (unvisited.length > 0) {
        const outerRadius = levels.length * RING_SPACING;
        for (let i = 0; i < unvisited.length; i++) {
            const angle = (2 * Math.PI * i) / unvisited.length;
            positions[unvisited[i]!.id] = {
                x: Math.cos(angle) * outerRadius,
                y: Math.sin(angle) * outerRadius,
            };
        }
    }

    return positions;
}
