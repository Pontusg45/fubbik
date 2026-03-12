import { useStore } from "@xyflow/react";
import { useMemo } from "react";

interface TagRegion {
    tagName: string;
    color: string;
    nodeIds: string[];
}

function convexHull(points: { x: number; y: number }[]): { x: number; y: number }[] {
    if (points.length <= 1) return points;
    const sorted = [...points].sort((a, b) => a.x - b.x || a.y - b.y);

    function cross(o: { x: number; y: number }, a: { x: number; y: number }, b: { x: number; y: number }) {
        return (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    }

    const lower: { x: number; y: number }[] = [];
    for (const p of sorted) {
        while (lower.length >= 2 && cross(lower[lower.length - 2]!, lower[lower.length - 1]!, p) <= 0) lower.pop();
        lower.push(p);
    }

    const upper: { x: number; y: number }[] = [];
    for (const p of [...sorted].reverse()) {
        while (upper.length >= 2 && cross(upper[upper.length - 2]!, upper[upper.length - 1]!, p) <= 0) upper.pop();
        upper.push(p);
    }

    lower.pop();
    upper.pop();
    return lower.concat(upper);
}

function hullToPath(hull: { x: number; y: number }[], padding: number): string {
    if (hull.length === 0) return "";
    if (hull.length === 1) {
        const p = hull[0]!;
        return `M ${p.x - padding} ${p.y} A ${padding} ${padding} 0 1 0 ${p.x + padding} ${p.y} A ${padding} ${padding} 0 1 0 ${p.x - padding} ${p.y} Z`;
    }
    if (hull.length === 2) {
        const [a, b] = hull as [{ x: number; y: number }, { x: number; y: number }];
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const len = Math.sqrt(dx * dx + dy * dy);
        const nx = (-dy / len) * padding;
        const ny = (dx / len) * padding;
        return `M ${a.x + nx} ${a.y + ny} L ${b.x + nx} ${b.y + ny} A ${padding} ${padding} 0 0 1 ${b.x - nx} ${b.y - ny} L ${a.x - nx} ${a.y - ny} A ${padding} ${padding} 0 0 1 ${a.x + nx} ${a.y + ny} Z`;
    }

    // Expand hull outward by padding
    const expanded: { x: number; y: number }[] = [];
    for (let i = 0; i < hull.length; i++) {
        const prev = hull[(i - 1 + hull.length) % hull.length]!;
        const curr = hull[i]!;
        const next = hull[(i + 1) % hull.length]!;

        const dx1 = curr.x - prev.x;
        const dy1 = curr.y - prev.y;
        const dx2 = next.x - curr.x;
        const dy2 = next.y - curr.y;

        const nx = -(dy1 + dy2) / 2;
        const ny = (dx1 + dx2) / 2;
        const len = Math.sqrt(nx * nx + ny * ny) || 1;

        expanded.push({ x: curr.x + (nx / len) * padding, y: curr.y + (ny / len) * padding });
    }

    let d = `M ${expanded[0]!.x} ${expanded[0]!.y}`;
    for (let i = 1; i < expanded.length; i++) {
        d += ` L ${expanded[i]!.x} ${expanded[i]!.y}`;
    }
    d += " Z";
    return d;
}

export function GraphTagRegions({
    regions,
    nodePositions
}: {
    regions: TagRegion[];
    nodePositions: Map<string, { x: number; y: number; width: number; height: number }>;
}) {
    const viewport = useStore(s => ({ x: s.transform[0], y: s.transform[1], zoom: s.transform[2] }));

    const paths = useMemo(() => {
        return regions
            .map(region => {
                const points: { x: number; y: number }[] = [];
                for (const nid of region.nodeIds) {
                    const pos = nodePositions.get(nid);
                    if (pos) {
                        points.push({ x: pos.x + pos.width / 2, y: pos.y + pos.height / 2 });
                    }
                }
                if (points.length === 0) return null;
                const hull = convexHull(points);
                const path = hullToPath(hull, 60);

                // Find top-left for label
                let labelX = Infinity;
                let labelY = Infinity;
                for (const p of points) {
                    if (p.y < labelY || (p.y === labelY && p.x < labelX)) {
                        labelX = p.x;
                        labelY = p.y;
                    }
                }

                return { ...region, path, labelX, labelY: labelY - 40 };
            })
            .filter(Boolean) as (TagRegion & { path: string; labelX: number; labelY: number })[];
    }, [regions, nodePositions]);

    return (
        <svg
            className="pointer-events-none absolute inset-0 z-0"
            style={{
                transform: `translate(${viewport.x}px, ${viewport.y}px) scale(${viewport.zoom})`,
                transformOrigin: "0 0"
            }}
        >
            {paths.map(region => (
                <g key={region.tagName}>
                    <path
                        d={region.path}
                        fill={region.color}
                        fillOpacity={0.06}
                        stroke={region.color}
                        strokeOpacity={0.2}
                        strokeWidth={1 / viewport.zoom}
                    />
                    <text
                        x={region.labelX}
                        y={region.labelY}
                        fill={region.color}
                        fillOpacity={0.6}
                        fontSize={12 / viewport.zoom}
                        fontWeight={600}
                        style={{ textTransform: "uppercase", letterSpacing: "0.1em" } as React.CSSProperties}
                    >
                        {region.tagName}
                    </text>
                </g>
            ))}
        </svg>
    );
}
