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
    const nodeSet = new Set(nodes.map(n => n.id));

    // Drop tags with zero visible members BEFORE computing first-wins. graph-view
    // does the same skip when rendering — if we diverge here, a chunk ends up
    // positioned in group A's grid (layout side) while rendered as a member of
    // group B (render side), stretching B's bounding box across the canvas.
    const visibleTagGroups = new Map<string, string[]>();
    for (const [name, nodeIds] of tagGroups) {
        const visible = nodeIds.filter(id => nodeSet.has(id));
        if (visible.length > 0) visibleTagGroups.set(name, visible);
    }

    // Build node-to-group mapping (first visible group wins)
    const nodeToGroup = new Map<string, string>();
    for (const [groupName, nodeIds] of visibleTagGroups) {
        for (const nid of nodeIds) {
            if (!nodeToGroup.has(nid)) nodeToGroup.set(nid, groupName);
        }
    }

    // Assign ungrouped nodes
    const ungroupedNodes: string[] = [];
    for (const n of nodes) {
        if (!nodeToGroup.has(n.id)) {
            ungroupedNodes.push(n.id);
            nodeToGroup.set(n.id, "__ungrouped__");
        }
    }
    const allGroups = new Map(visibleTagGroups);
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

    // Grid cell size — sized to the actual measured chunk nodes (~180×36) with
    // enough padding to keep labels readable. Previously 280×80 which left the grid
    // ~2-3× wider than tall and wasted lots of space inside each group box.
    const NODE_W = 200;
    const NODE_H = 72;

    // Pick column count so that the rendered grid approximates a square *visual*
    // block, not a square *count* grid. With cells taller in aspect than wide
    // (NODE_H/NODE_W), we need more rows than columns to balance out the
    // per-cell footprint:
    //   cols * NODE_W ≈ rows * NODE_H   ⇒   cols ≈ sqrt(n * NODE_H / NODE_W)
    // This keeps group boxes compact across any chunk count.
    function cellsForCount(n: number): { cols: number; rows: number } {
        if (n <= 1) return { cols: 1, rows: n };
        const cols = Math.max(1, Math.ceil(Math.sqrt(n * (NODE_H / NODE_W))));
        const rows = Math.ceil(n / cols);
        return { cols, rows };
    }

    // Authoritative per-group membership from nodeToGroup (first-wins, matches
    // graph-view's chunkToGroupId). Using allGroups directly here would double-count
    // chunks that carry multiple tags, making groupRadius + Phase 2 placement
    // overlap-overwrite each other.
    const groupMembers = new Map<string, string[]>();
    for (const [chunkId, groupName] of nodeToGroup) {
        if (!nodeSet.has(chunkId)) continue;
        if (!groupMembers.has(groupName)) groupMembers.set(groupName, []);
        groupMembers.get(groupName)!.push(chunkId);
    }

    // Pre-compute group radii based on grid layout footprint
    const groupRadius = new Map<string, number>();
    for (const name of groupNames) {
        const members = groupMembers.get(name) ?? [];
        if (members.length === 0) { groupRadius.set(name, 0); continue; }
        const { cols, rows } = cellsForCount(members.length);
        const w = cols * NODE_W / 2;
        const h = rows * NODE_H / 2;
        groupRadius.set(name, Math.sqrt(w * w + h * h) + 60); // 60px padding
    }

    const GROUP_REPULSION = 200000;
    const GROUP_SPRING_K = 0.002;
    const GROUP_SPRING_LEN = 350;
    const GROUP_DAMPING = 0.85;
    const GROUP_ITERATIONS = 200;

    const OVERLAP_PAD = 40; // extra gutter between group boxes

    for (let iter = 0; iter < GROUP_ITERATIONS; iter++) {
        const temp = 1 - iter / GROUP_ITERATIONS;

        // Repulsion between groups (with strong overlap prevention)
        for (let i = 0; i < groupNames.length; i++) {
            for (let j = i + 1; j < groupNames.length; j++) {
                const a = groupPos.get(groupNames[i]!)!;
                const b = groupPos.get(groupNames[j]!)!;
                const dx = b.x - a.x;
                const dy = b.y - a.y;
                const dist = Math.sqrt(dx * dx + dy * dy) + 1;
                const rA = groupRadius.get(groupNames[i]!) ?? 100;
                const rB = groupRadius.get(groupNames[j]!) ?? 100;
                const minDist = rA + rB + OVERLAP_PAD;

                // Standard inverse-square repulsion scaled by group sizes.
                const sizeA = allGroups.get(groupNames[i]!)!.length;
                const sizeB = allGroups.get(groupNames[j]!)!.length;
                const sizeFactor = Math.sqrt(sizeA * sizeB);
                let force = (GROUP_REPULSION * sizeFactor * temp) / (dist * dist);

                // Hard separation when groups would visually overlap. This used to be
                // too weak (0.5) to fight springs pulling big, highly-connected groups
                // together — the result was three giant boxes stacked at the origin.
                // 2.0 dominates the spring force at reasonable separations.
                if (dist < minDist) {
                    force += (minDist - dist) * 2.0;
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
            // Target spring length must be at least sum-of-radii so the spring's pull
            // doesn't drag big groups through each other. Before, it was min 350px
            // regardless of how big the groups actually drew.
            const avgLen = relations.reduce((sum, r) => sum + (RELATION_SPRING_LEN[r] ?? SPRING_LEN), 0) / relations.length;
            const rA = groupRadius.get(gA!) ?? 100;
            const rB = groupRadius.get(gB!) ?? 100;
            const separation = rA + rB + OVERLAP_PAD;
            const targetLen = Math.max(GROUP_SPRING_LEN, avgLen * 1.5, separation);
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

    // Phase 1.5: Hard collision resolution pass. The soft forces above may leave
    // some groups still overlapping because (a) the annealing temperature is
    // zero at the end so late-phase adjustments are tiny, and (b) with many
    // groups pulling on each other, springs can lock overlaps in. This pass
    // walks pairs and pushes them apart by half the penetration each — no force,
    // no damping, just geometry. Repeats a few times so cascades settle.
    for (let pass = 0; pass < 40; pass++) {
        let moved = false;
        for (let i = 0; i < groupNames.length; i++) {
            for (let j = i + 1; j < groupNames.length; j++) {
                const a = groupPos.get(groupNames[i]!)!;
                const b = groupPos.get(groupNames[j]!)!;
                const rA = groupRadius.get(groupNames[i]!) ?? 0;
                const rB = groupRadius.get(groupNames[j]!) ?? 0;
                if (rA === 0 || rB === 0) continue;
                const dx = b.x - a.x;
                const dy = b.y - a.y;
                const dist = Math.sqrt(dx * dx + dy * dy) + 0.0001;
                const minDist = rA + rB + OVERLAP_PAD;
                if (dist >= minDist) continue;
                const overlap = minDist - dist;
                const nx = dx / dist;
                const ny = dy / dist;
                // Push each group by half the overlap along the separation axis.
                a.x -= nx * overlap * 0.5;
                a.y -= ny * overlap * 0.5;
                b.x += nx * overlap * 0.5;
                b.y += ny * overlap * 0.5;
                moved = true;
            }
        }
        if (!moved) break;
    }

    // Phase 2: Place each chunk exactly once, in its first-wins group's grid.
    // (Using allGroups here would double-place chunks that belong to multiple tags
    // — the last group in iteration would win, leaving chunks far from their
    // "visual" group's center and stretching bounds across the whole canvas.)
    const positions: Record<string, { x: number; y: number }> = {};
    for (const [groupName, members] of groupMembers) {
        const gp = groupPos.get(groupName);
        if (!gp || members.length === 0) continue;
        const { cols, rows } = cellsForCount(members.length);
        for (let i = 0; i < members.length; i++) {
            const col = i % cols;
            const row = Math.floor(i / cols);
            positions[members[i]!] = {
                x: gp.x + (col - (cols - 1) / 2) * NODE_W,
                y: gp.y + (row - (rows - 1) / 2) * NODE_H
            };
        }
    }

    return positions;
}
