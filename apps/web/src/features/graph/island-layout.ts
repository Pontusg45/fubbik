export interface IslandLayoutInput {
    islands: Array<{ id: string; chunkCount: number }>;
    bridges: Array<{ fromIslandId: string; toIslandId: string; count: number }>;
}

const REPULSION = 50000;
const SPRING_K = 0.02;
const SPRING_REST = 200;
const CENTER_PULL = 0.01;
const DAMPING = 0.8;
const ITERATIONS = 120;

export function layoutIslands(input: IslandLayoutInput): Record<string, { x: number; y: number }> {
    const { islands, bridges } = input;
    if (islands.length === 0) return {};
    if (islands.length === 1) return { [islands[0]!.id]: { x: 0, y: 0 } };

    const state = new Map<string, { x: number; y: number; vx: number; vy: number }>();
    const radius = islands.length * 40;
    for (let i = 0; i < islands.length; i++) {
        const angle = (2 * Math.PI * i) / islands.length;
        state.set(islands[i]!.id, {
            x: Math.cos(angle) * radius,
            y: Math.sin(angle) * radius,
            vx: 0,
            vy: 0,
        });
    }

    for (let iter = 0; iter < ITERATIONS; iter++) {
        for (let i = 0; i < islands.length; i++) {
            const a = state.get(islands[i]!.id)!;
            for (let j = i + 1; j < islands.length; j++) {
                const b = state.get(islands[j]!.id)!;
                const dx = a.x - b.x;
                const dy = a.y - b.y;
                const dist = Math.max(Math.hypot(dx, dy), 1);
                const force = REPULSION / (dist * dist);
                const fx = (dx / dist) * force;
                const fy = (dy / dist) * force;
                a.vx += fx; a.vy += fy;
                b.vx -= fx; b.vy -= fy;
            }
        }

        for (const bridge of bridges) {
            const a = state.get(bridge.fromIslandId);
            const b = state.get(bridge.toIslandId);
            if (!a || !b) continue;
            const dx = b.x - a.x;
            const dy = b.y - a.y;
            const dist = Math.max(Math.hypot(dx, dy), 1);
            const displacement = dist - SPRING_REST;
            const force = SPRING_K * displacement * Math.min(bridge.count, 10);
            const fx = (dx / dist) * force;
            const fy = (dy / dist) * force;
            a.vx += fx; a.vy += fy;
            b.vx -= fx; b.vy -= fy;
        }

        for (const s of state.values()) {
            s.vx -= s.x * CENTER_PULL;
            s.vy -= s.y * CENTER_PULL;
        }

        for (const s of state.values()) {
            s.x += s.vx; s.y += s.vy;
            s.vx *= DAMPING; s.vy *= DAMPING;
        }
    }

    const positions: Record<string, { x: number; y: number }> = {};
    for (const [id, s] of state) {
        positions[id] = { x: Math.round(s.x), y: Math.round(s.y) };
    }
    return positions;
}
