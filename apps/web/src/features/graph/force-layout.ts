import { buildQuadtree, computeRepulsion } from "./quadtree";

const MAIN_NODE_ID = "__main__";

const REPULSION = 160000;
const SPRING_K = 0.002;
const CENTER_PULL = 0.003;
const DAMPING = 0.85;
const ITERATIONS = 200;
const CLUSTER_K = 0.001;

const SPRING_LEN = 450;
const RELATION_SPRING_LEN: Record<string, number> = {
    part_of: 300,
    depends_on: 380,
    extends: 380,
    references: 450,
    related_to: 520,
    supports: 450,
    contradicts: 560,
    alternative_to: 560
};

export function runForceLayout(
    nodes: { id: string; type: string }[],
    edges: { source: string; target: string; relation: string }[],
    tagGroups?: Map<string, string[]> // tagValue -> nodeIds in that group
): Record<string, { x: number; y: number }> {
    const nodeCount = nodes.length;
    const spacing = Math.max(400, Math.sqrt(nodeCount) * 180);

    const pos = new Map<string, { x: number; y: number; vx: number; vy: number }>();
    for (let i = 0; i < nodes.length; i++) {
        const id = nodes[i]!.id;
        if (id === MAIN_NODE_ID) {
            pos.set(id, { x: 0, y: 0, vx: 0, vy: 0 });
        } else {
            const angle = (2 * Math.PI * i) / nodes.length;
            const r = spacing * 0.8;
            pos.set(id, { x: Math.cos(angle) * r, y: Math.sin(angle) * r, vx: 0, vy: 0 });
        }
    }

    const nodeType = new Map<string, string>();
    for (const n of nodes) {
        nodeType.set(n.id, n.type);
    }

    const edgePairs: [string, string, number][] = edges.map(e => {
        const len = RELATION_SPRING_LEN[e.relation] ?? SPRING_LEN;
        return [e.source, e.target, len];
    });

    for (let iter = 0; iter < ITERATIONS; iter++) {
        const temp = 1 - iter / ITERATIONS;

        const ids = [...pos.keys()];
        const bodies = ids.map(id => ({ id, x: pos.get(id)!.x, y: pos.get(id)!.y }));
        const tree = buildQuadtree(bodies);
        if (tree) {
            for (const id of ids) {
                const p = pos.get(id)!;
                const { fx, fy } = computeRepulsion(tree, p.x, p.y, REPULSION * temp);
                p.vx += fx;
                p.vy += fy;
            }
        }

        for (const [srcId, tgtId, targetLen] of edgePairs) {
            const a = pos.get(srcId);
            const b = pos.get(tgtId);
            if (!a || !b) continue;
            const dx = b.x - a.x;
            const dy = b.y - a.y;
            const dist = Math.sqrt(dx * dx + dy * dy) + 1;
            const force = SPRING_K * (dist - targetLen) * temp;
            const fx = (dx / dist) * force;
            const fy = (dy / dist) * force;
            a.vx += fx;
            a.vy += fy;
            b.vx -= fx;
            b.vy -= fy;
        }

        const typeCentroids = new Map<string, { x: number; y: number; count: number }>();
        for (const [id, p] of pos) {
            const t = nodeType.get(id);
            if (!t) continue;
            const c = typeCentroids.get(t) ?? { x: 0, y: 0, count: 0 };
            c.x += p.x;
            c.y += p.y;
            c.count++;
            typeCentroids.set(t, c);
        }
        for (const [id, p] of pos) {
            const t = nodeType.get(id);
            if (!t) continue;
            const c = typeCentroids.get(t)!;
            const cx = c.x / c.count;
            const cy = c.y / c.count;
            p.vx += (cx - p.x) * CLUSTER_K * temp;
            p.vy += (cy - p.y) * CLUSTER_K * temp;
        }

        // Tag-based clustering (when grouping is active)
        if (tagGroups && tagGroups.size > 0) {
            const TAG_CLUSTER_K = 0.003;
            const tagCentroids = new Map<string, { x: number; y: number; count: number }>();
            for (const [tagValue, nodeIds] of tagGroups) {
                let cx = 0, cy = 0, count = 0;
                for (const nid of nodeIds) {
                    const p = pos.get(nid);
                    if (p) { cx += p.x; cy += p.y; count++; }
                }
                if (count > 0) tagCentroids.set(tagValue, { x: cx / count, y: cy / count, count });
            }
            for (const [tagValue, nodeIds] of tagGroups) {
                const c = tagCentroids.get(tagValue);
                if (!c) continue;
                for (const nid of nodeIds) {
                    const p = pos.get(nid);
                    if (!p) continue;
                    p.vx += (c.x - p.x) * TAG_CLUSTER_K * temp;
                    p.vy += (c.y - p.y) * TAG_CLUSTER_K * temp;
                }
            }
        }

        for (const p of pos.values()) {
            p.vx -= p.x * CENTER_PULL * temp;
            p.vy -= p.y * CENTER_PULL * temp;
        }

        for (const [id, p] of pos) {
            if (id === MAIN_NODE_ID) {
                p.vx = 0;
                p.vy = 0;
                continue;
            }
            p.x += p.vx;
            p.y += p.vy;
            p.vx *= DAMPING;
            p.vy *= DAMPING;
        }
    }

    const positions: Record<string, { x: number; y: number }> = {};
    for (const [id, p] of pos) {
        positions[id] = { x: p.x, y: p.y };
    }
    return positions;
}
