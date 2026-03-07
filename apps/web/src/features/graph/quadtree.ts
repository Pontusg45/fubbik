interface QTNode {
    x: number;
    y: number;
    mass: number;
    x0: number;
    y0: number;
    x1: number;
    y1: number;
    children: (QTNode | null)[];
    bodyX?: number;
    bodyY?: number;
    bodyId?: string;
    isLeaf: boolean;
}

export function buildQuadtree(bodies: { id: string; x: number; y: number }[]): QTNode | null {
    if (bodies.length === 0) return null;

    let minX = Infinity,
        minY = Infinity,
        maxX = -Infinity,
        maxY = -Infinity;
    for (const b of bodies) {
        if (b.x < minX) minX = b.x;
        if (b.y < minY) minY = b.y;
        if (b.x > maxX) maxX = b.x;
        if (b.y > maxY) maxY = b.y;
    }
    const size = Math.max(maxX - minX, maxY - minY, 1) + 100;
    const cx = (minX + maxX) / 2;
    const cy = (minY + maxY) / 2;

    const root = createNode(cx - size / 2, cy - size / 2, cx + size / 2, cy + size / 2);
    for (const b of bodies) {
        insert(root, b.x, b.y, b.id);
    }
    return root;
}

function createNode(x0: number, y0: number, x1: number, y1: number): QTNode {
    return { x: 0, y: 0, mass: 0, x0, y0, x1, y1, children: [null, null, null, null], isLeaf: true };
}

function insert(node: QTNode, x: number, y: number, id: string): void {
    if (node.mass === 0) {
        node.x = x;
        node.y = y;
        node.mass = 1;
        node.bodyX = x;
        node.bodyY = y;
        node.bodyId = id;
        node.isLeaf = true;
        return;
    }

    if (node.isLeaf) {
        node.children = [
            createNode(node.x0, node.y0, (node.x0 + node.x1) / 2, (node.y0 + node.y1) / 2),
            createNode((node.x0 + node.x1) / 2, node.y0, node.x1, (node.y0 + node.y1) / 2),
            createNode(node.x0, (node.y0 + node.y1) / 2, (node.x0 + node.x1) / 2, node.y1),
            createNode((node.x0 + node.x1) / 2, (node.y0 + node.y1) / 2, node.x1, node.y1)
        ];
        insertIntoChild(node, node.bodyX!, node.bodyY!, node.bodyId!);
        node.bodyX = undefined;
        node.bodyY = undefined;
        node.bodyId = undefined;
        node.isLeaf = false;
    }

    insertIntoChild(node, x, y, id);
    const totalMass = node.mass + 1;
    node.x = (node.x * node.mass + x) / totalMass;
    node.y = (node.y * node.mass + y) / totalMass;
    node.mass = totalMass;
}

function insertIntoChild(node: QTNode, x: number, y: number, id: string): void {
    const midX = (node.x0 + node.x1) / 2;
    const midY = (node.y0 + node.y1) / 2;
    const idx = (x < midX ? 0 : 1) + (y < midY ? 0 : 2);
    insert(node.children[idx]!, x, y, id);
}

export function computeRepulsion(
    root: QTNode,
    bx: number,
    by: number,
    repulsion: number,
    theta: number = 0.9
): { fx: number; fy: number } {
    let fx = 0;
    let fy = 0;

    function walk(node: QTNode | null): void {
        if (!node || node.mass === 0) return;

        const dx = bx - node.x;
        const dy = by - node.y;
        const distSq = dx * dx + dy * dy + 1;
        const dist = Math.sqrt(distSq);

        if (node.isLeaf) {
            if (dist > 0.1) {
                const force = repulsion / distSq;
                fx += (dx / dist) * force;
                fy += (dy / dist) * force;
            }
            return;
        }

        const cellSize = node.x1 - node.x0;
        if (cellSize / dist < theta) {
            const force = (repulsion * node.mass) / distSq;
            fx += (dx / dist) * force;
            fy += (dy / dist) * force;
            return;
        }

        for (const child of node.children) {
            walk(child);
        }
    }

    walk(root);
    return { fx, fy };
}
