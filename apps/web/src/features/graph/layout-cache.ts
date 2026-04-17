/**
 * Session-scoped cache of computed graph layouts.
 *
 * The worker's force simulation is deterministic-ish (seeded ring layout, fixed
 * iteration counts) but expensive — 400+ ms on the seeded 140-chunk graph, and
 * scaling roughly linearly with nodes + edges. Most user-visible latency comes
 * from the same inputs being laid out twice: toggle filter A → B → A returns to
 * the exact same graph but pays the full layout cost again.
 *
 * This cache keys on the structural inputs (node IDs, edge IDs, grouping mode,
 * algorithm) so the same filter combo returns instantly from storage. It deliberately
 * lives in sessionStorage rather than localStorage — positions shouldn't outlive
 * the tab, and we don't want stale positions to survive a schema change.
 */

const STORAGE_KEY = "fubbik:graph-layout-cache:v1";
const MAX_ENTRIES = 20;

type Positions = Record<string, { x: number; y: number }>;

interface CacheEntry {
    key: string;
    positions: Positions;
    storedAt: number;
}

function read(): CacheEntry[] {
    if (typeof sessionStorage === "undefined") return [];
    try {
        const raw = sessionStorage.getItem(STORAGE_KEY);
        if (!raw) return [];
        const parsed = JSON.parse(raw) as unknown;
        return Array.isArray(parsed) ? (parsed as CacheEntry[]) : [];
    } catch {
        return [];
    }
}

function write(entries: CacheEntry[]) {
    if (typeof sessionStorage === "undefined") return;
    try {
        sessionStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
    } catch {
        // Quota exceeded — drop the oldest half and retry once. If that fails,
        // give up silently; the cache is a perf optimization, not a correctness
        // requirement.
        try {
            const half = entries.slice(-Math.floor(entries.length / 2));
            sessionStorage.setItem(STORAGE_KEY, JSON.stringify(half));
        } catch {
            // ignore
        }
    }
}

/**
 * Build a stable key for a layout run. `nodeIds` and `edgeIds` MUST already
 * be sorted so toggles that produce the same set (but different insertion order)
 * still hit the cache.
 */
export function layoutCacheKey(params: {
    layoutAlgorithm: string;
    groupingMode: string | null;
    nodeIds: string[];
    edgeIds: string[];
}): string {
    return [
        params.layoutAlgorithm,
        params.groupingMode ?? "none",
        // Pre-sort callers should sort, but do it defensively.
        [...params.nodeIds].sort().join("|"),
        [...params.edgeIds].sort().join("|")
    ].join("::");
}

export function getCachedLayout(key: string): Positions | null {
    const entries = read();
    const hit = entries.find(e => e.key === key);
    if (!hit) return null;
    // Touch: move hit to the end so LRU eviction keeps it around.
    const rest = entries.filter(e => e.key !== key);
    rest.push({ ...hit, storedAt: Date.now() });
    write(rest);
    return hit.positions;
}

export function setCachedLayout(key: string, positions: Positions): void {
    const entries = read().filter(e => e.key !== key);
    entries.push({ key, positions, storedAt: Date.now() });
    // LRU eviction: oldest entries at the front.
    while (entries.length > MAX_ENTRIES) entries.shift();
    write(entries);
}

export function clearLayoutCache(): void {
    if (typeof sessionStorage === "undefined") return;
    try {
        sessionStorage.removeItem(STORAGE_KEY);
    } catch {
        // ignore
    }
}
