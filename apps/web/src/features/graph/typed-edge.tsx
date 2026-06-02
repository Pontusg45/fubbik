import { type EdgeProps, useInternalNode } from "@xyflow/react";
import { memo } from "react";

interface TypedEdgeData {
    relation: string;
    [key: string]: unknown;
}

const EDGE_STYLES: Record<string, { color: string; strokeWidth: number; strokeDasharray?: string; markerEnd?: string }> = {
    depends_on: { color: "#3b82f6", strokeWidth: 2, markerEnd: "arrow-filled" },
    part_of: { color: "#22c55e", strokeWidth: 2.5, markerEnd: "dot" },
    extends: { color: "#a78bfa", strokeWidth: 1.5, markerEnd: "arrow-open" },
    references: { color: "#94a3b8", strokeWidth: 1, strokeDasharray: "6 4" },
    related_to: { color: "#94a3b8", strokeWidth: 1, strokeDasharray: "6 4" },
    contradicts: { color: "#ef4444", strokeWidth: 2, strokeDasharray: "4 4" },
    alternative_to: { color: "#f59e0b", strokeWidth: 1.5 },
    supports: { color: "#06b6d4", strokeWidth: 1.5, strokeDasharray: "8 3" }
};

const DEFAULT_STYLE: { color: string; strokeWidth: number; strokeDasharray?: string; markerEnd?: string } = {
    color: "#6b7280",
    strokeWidth: 1
};

function getNodeCenter(node: { position: { x: number; y: number }; measured?: { width?: number; height?: number } }) {
    const w = node.measured?.width ?? 100;
    const h = node.measured?.height ?? 40;
    return { x: node.position.x + w / 2, y: node.position.y + h / 2, w, h };
}

function getEdgePoint(center: { x: number; y: number; w: number; h: number }, otherCenter: { x: number; y: number }) {
    const dx = otherCenter.x - center.x;
    const dy = otherCenter.y - center.y;
    const hw = center.w / 2;
    const hh = center.h / 2;

    if (dx === 0 && dy === 0) return { x: center.x, y: center.y };

    const absDx = Math.abs(dx);
    const absDy = Math.abs(dy);
    const ratio = hw / hh;

    let x: number;
    let y: number;
    if (absDx / absDy > ratio) {
        const sign = dx > 0 ? 1 : -1;
        x = center.x + sign * hw;
        y = center.y + (dy / absDx) * hw;
    } else {
        const sign = dy > 0 ? 1 : -1;
        y = center.y + sign * hh;
        x = center.x + (dx / absDy) * hh;
    }

    return { x, y };
}

function TypedEdgeComponent({ id, source, target, data, style }: EdgeProps) {
    const sourceNode = useInternalNode(source);
    const targetNode = useInternalNode(target);

    if (!sourceNode || !targetNode) return null;

    const relation = (data as TypedEdgeData | undefined)?.relation ?? "related_to";
    const edgeStyle = EDGE_STYLES[relation] ?? DEFAULT_STYLE;

    const sc = getNodeCenter(sourceNode);
    const tc = getNodeCenter(targetNode);
    const sp = getEdgePoint(sc, tc);
    const tp = getEdgePoint(tc, sc);

    const dx = tp.x - sp.x;
    const dy = tp.y - sp.y;
    const dist = Math.hypot(dx, dy);
    const curvature = Math.min(dist * 0.25, 80);
    const mx = (sp.x + tp.x) / 2;
    const my = (sp.y + tp.y) / 2;
    const nx = -dy / (dist || 1);
    const ny = dx / (dist || 1);
    const cx = mx + nx * curvature * 0.15;
    const cy = my + ny * curvature * 0.15;

    const path = `M ${sp.x} ${sp.y} Q ${cx} ${cy} ${tp.x} ${tp.y}`;

    const markerId = edgeStyle.markerEnd ? `${edgeStyle.markerEnd}-${id}` : undefined;

    return (
        <g>
            <defs>
                <marker id={`arrow-filled-${id}`} markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto">
                    <polygon points="0 0, 8 3, 0 6" fill={edgeStyle.color} />
                </marker>
                <marker id={`arrow-open-${id}`} markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto">
                    <path d="M0,0 L8,3 L0,6" fill="none" stroke={edgeStyle.color} strokeWidth="1.5" />
                </marker>
                <marker id={`dot-${id}`} markerWidth="8" markerHeight="8" refX="4" refY="4" orient="auto">
                    <circle cx="4" cy="4" r="3" fill="none" stroke={edgeStyle.color} strokeWidth="1.5" />
                </marker>
            </defs>
            {/* Invisible fat path for easier hover/click */}
            <path d={path} fill="none" stroke="transparent" strokeWidth={20} />
            <path
                d={path}
                fill="none"
                stroke={edgeStyle.color}
                strokeWidth={edgeStyle.strokeWidth}
                strokeDasharray={edgeStyle.strokeDasharray}
                markerEnd={markerId ? `url(#${markerId})` : undefined}
                style={style}
            />
        </g>
    );
}

export const TypedEdge = memo(TypedEdgeComponent);
export default TypedEdge;
