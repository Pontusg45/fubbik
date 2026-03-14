import { buildQuadtree, computeRepulsion } from "./quadtree";

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
    // When tag groups are active, use two-phase layout:
    // Phase 1: Position groups as single nodes
    // Phase 2: Arrange nodes within each group
    if (tagGroups && tagGroups.size > 0) {
        return runGroupedLayout(nodes, edges, tagGroups);
    }

    return runUngroupedLayout(nodes, edges);
}

function runUngroupedLayout(
    nodes: { id: string; type: string }[],
    edges: { source: string; target: string; relation: string }[]
): Record<string, { x: number; y: number }> {
    const nodeCount = nodes.length;
    const spacing = Math.max(400, Math.sqrt(nodeCount) * 180);

    const pos = new Map<string, { x: number; y: number; vx: number; vy: number }>();
    for (let i = 0; i < nodes.length; i++) {
        const id = nodes[i]!.id;
        const angle = (2 * Math.PI * i) / nodes.length;
        const r = spacing * 0.8;
        pos.set(id, { x: Math.cos(angle) * r, y: Math.sin(angle) * r, vx: 0, vy: 0 });
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

        for (const p of pos.values()) {
            p.vx -= p.x * CENTER_PULL * temp;
            p.vy -= p.y * CENTER_PULL * temp;
        }

        for (const p of pos.values()) {
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

function runGroupedLayout(
    nodes: { id: string; type: string }[],
    edges: { source: string; target: string; relation: string }[],
    tagGroups: Map<string, string[]>
): Record<string, { x: number; y: number }> {
    // Build node-to-group mapping (first group wins)
    const nodeToGroup = new Map<string, string>();
    for (const [groupName, nodeIds] of tagGroups) {
        for (const nid of nodeIds) {
            if (!nodeToGroup.has(nid)) nodeToGroup.set(nid, groupName);
        }
    }

    // Assign ungrouped nodes
    const nodeSet = new Set(nodes.map(n => n.id));
    const ungroupedNodes: string[] = [];
    for (const n of nodes) {
        if (!nodeToGroup.has(n.id)) {
            ungroupedNodes.push(n.id);
            nodeToGroup.set(n.id, "__ungrouped__");
        }
    }
    const allGroups = new Map(tagGroups);
    if (ungroupedNodes.length > 0) {
        allGroups.set("__ungrouped__", ungroupedNodes);
    }

    // Phase 1: Position groups using force layout on group-level graph
    const groupNames = [...allGroups.keys()];
    const groupCount = groupNames.length;

    // Count inter-group edges to determine spring strengths
    const groupEdges = new Map<string, { count: number; relations: string[] }>();
    for (const e of edges) {
        const gSrc = nodeToGroup.get(e.source);
        const gTgt = nodeToGroup.get(e.target);
        if (!gSrc || !gTgt || gSrc === gTgt) continue;
        const key = [gSrc, gTgt].sort().join("||");
        const existing = groupEdges.get(key);
        if (existing) {
            existing.count++;
            existing.relations.push(e.relation);
        } else {
            groupEdges.set(key, { count: 1, relations: [e.relation] });
        }
    }

    // Run force layout for groups
    const groupSpacing = Math.max(400, Math.sqrt(groupCount) * 250);
    const groupPos = new Map<string, { x: number; y: number; vx: number; vy: number }>();
    for (let i = 0; i < groupNames.length; i++) {
        const angle = (2 * Math.PI * i) / groupNames.length;
        const r = groupSpacing * 0.6;
        groupPos.set(groupNames[i]!, { x: Math.cos(angle) * r, y: Math.sin(angle) * r, vx: 0, vy: 0 });
    }

    const NODE_W = 280;
    const NODE_H = 80;

    // Pre-compute group radii based on grid layout footprint
    const groupRadius = new Map<string, number>();
    for (const [name, nodeIds] of allGroups) {
        const n = nodeIds.filter(id => nodeSet.has(id)).length;
        if (n === 0) { groupRadius.set(name, 0); continue; }
        const cols = Math.ceil(Math.sqrt(n));
        const rows = Math.ceil(n / cols);
        const w = cols * NODE_W / 2;
        const h = rows * NODE_H / 2;
        groupRadius.set(name, Math.sqrt(w * w + h * h) + 60); // 60px padding
    }

    const GROUP_REPULSION = 200000;
    const GROUP_SPRING_K = 0.002;
    const GROUP_SPRING_LEN = 350;
    const GROUP_DAMPING = 0.85;
    const GROUP_ITERATIONS = 200;

    for (let iter = 0; iter < GROUP_ITERATIONS; iter++) {
        const temp = 1 - iter / GROUP_ITERATIONS;

        // Repulsion between groups (with overlap prevention)
        for (let i = 0; i < groupNames.length; i++) {
            for (let j = i + 1; j < groupNames.length; j++) {
                const a = groupPos.get(groupNames[i]!)!;
                const b = groupPos.get(groupNames[j]!)!;
                const dx = b.x - a.x;
                const dy = b.y - a.y;
                const dist = Math.sqrt(dx * dx + dy * dy) + 1;
                const rA = groupRadius.get(groupNames[i]!) ?? 100;
                const rB = groupRadius.get(groupNames[j]!) ?? 100;
                const minDist = rA + rB;

                // Standard repulsion
                const sizeA = allGroups.get(groupNames[i]!)!.length;
                const sizeB = allGroups.get(groupNames[j]!)!.length;
                const sizeFactor = Math.sqrt(sizeA * sizeB);
                let force = (GROUP_REPULSION * sizeFactor * temp) / (dist * dist);

                // Strong overlap separation force when groups are too close
                if (dist < minDist) {
                    force += (minDist - dist) * 0.5;
                }

                const fx = (dx / dist) * force;
                const fy = (dy / dist) * force;
                a.vx -= fx;
                a.vy -= fy;
                b.vx += fx;
                b.vy += fy;
            }
        }

        // Spring attraction for connected groups
        for (const [key, { count, relations }] of groupEdges) {
            const [gA, gB] = key.split("||");
            const a = groupPos.get(gA!);
            const b = groupPos.get(gB!);
            if (!a || !b) continue;
            const dx = b.x - a.x;
            const dy = b.y - a.y;
            const dist = Math.sqrt(dx * dx + dy * dy) + 1;
            // Average spring length from relations
            const avgLen = relations.reduce((sum, r) => sum + (RELATION_SPRING_LEN[r] ?? SPRING_LEN), 0) / relations.length;
            const targetLen = Math.max(GROUP_SPRING_LEN, avgLen * 1.5);
            const force = GROUP_SPRING_K * (dist - targetLen) * Math.min(count, 5) * temp;
            const fx = (dx / dist) * force;
            const fy = (dy / dist) * force;
            a.vx += fx;
            a.vy += fy;
            b.vx -= fx;
            b.vy -= fy;
        }

        // Center pull
        for (const p of groupPos.values()) {
            p.vx -= p.x * CENTER_PULL * temp;
            p.vy -= p.y * CENTER_PULL * temp;
        }

        // Apply velocities
        for (const p of groupPos.values()) {
            p.x += p.vx;
            p.y += p.vy;
            p.vx *= GROUP_DAMPING;
            p.vy *= GROUP_DAMPING;
        }
    }

    // Phase 2: Place nodes in a square grid within each group
    const positions: Record<string, { x: number; y: number }> = {};

    for (const [groupName, nodeIds] of allGroups) {
        const gp = groupPos.get(groupName);
        if (!gp) continue;

        const validIds = nodeIds.filter(id => nodeSet.has(id));
        if (validIds.length === 0) continue;

        const cols = Math.ceil(Math.sqrt(validIds.length));
        const rows = Math.ceil(validIds.length / cols);

        for (let i = 0; i < validIds.length; i++) {
            const col = i % cols;
            const row = Math.floor(i / cols);
            positions[validIds[i]!] = {
                x: gp.x + (col - (cols - 1) / 2) * NODE_W,
                y: gp.y + (row - (rows - 1) / 2) * NODE_H
            };
        }
    }

    return positions;
}
