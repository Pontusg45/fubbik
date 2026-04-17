/**
 * Lightweight performance marks for the /graph pipeline.
 *
 * Three milestones worth tracking:
 *   - time-to-first-node: route mount → first node rendered
 *   - layout-duration:    worker dispatch → positions in state
 *   - react-flow-paint:   positions in state → React Flow painted them
 *
 * Marks cost ~microseconds and only log in dev. Search devtools for "[graph-perf]".
 */

const PREFIX = "graph-perf";
const isDev = typeof import.meta !== "undefined" && (import.meta as { env?: { DEV?: boolean } }).env?.DEV;

export function mark(name: string) {
    if (!isDev || typeof performance === "undefined") return;
    try {
        performance.mark(`${PREFIX}:${name}`);
    } catch {
        // performance API may be disabled in some sandboxes
    }
}

export function measure(name: string, startMark: string, endMark: string) {
    if (!isDev || typeof performance === "undefined") return;
    try {
        const entry = performance.measure(
            `${PREFIX}:${name}`,
            `${PREFIX}:${startMark}`,
            `${PREFIX}:${endMark}`
        );
        // eslint-disable-next-line no-console
        console.log(`[${PREFIX}] ${name}: ${Math.round(entry.duration)}ms`);
    } catch {
        // marks may not exist yet — measure silently skips
    }
}

export function clearMarks() {
    if (typeof performance === "undefined") return;
    try {
        performance.clearMarks();
        performance.clearMeasures();
    } catch {
        // ignore
    }
}
