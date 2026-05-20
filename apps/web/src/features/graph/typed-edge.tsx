import { memo } from "react";
import { BaseEdge, type EdgeProps, getBezierPath } from "@xyflow/react";

interface TypedEdgeData {
    relation: string;
    [key: string]: unknown;
}

const EDGE_STYLES: Record<string, {
    color: string;
    strokeWidth: number;
    strokeDasharray?: string;
    markerEnd?: string;
}> = {
    depends_on:     { color: "#3b82f6", strokeWidth: 2, markerEnd: "arrow-filled" },
    part_of:        { color: "#22c55e", strokeWidth: 2.5, markerEnd: "dot" },
    extends:        { color: "#a78bfa", strokeWidth: 1.5, markerEnd: "arrow-open" },
    references:     { color: "#94a3b8", strokeWidth: 1, strokeDasharray: "6 4" },
    related_to:     { color: "#94a3b8", strokeWidth: 1, strokeDasharray: "6 4" },
    contradicts:    { color: "#ef4444", strokeWidth: 2, strokeDasharray: "4 4", markerEnd: "slash" },
    alternative_to: { color: "#f59e0b", strokeWidth: 1.5, markerEnd: "fork" },
    supports:       { color: "#06b6d4", strokeWidth: 1.5, strokeDasharray: "8 3" },
};

const DEFAULT_STYLE = { color: "#6b7280", strokeWidth: 1, strokeDasharray: undefined, markerEnd: undefined };

function TypedEdgeComponent({
    id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data, style,
}: EdgeProps) {
    const relation = (data as TypedEdgeData | undefined)?.relation ?? "related_to";
    const edgeStyle = EDGE_STYLES[relation] ?? DEFAULT_STYLE;

    const [edgePath] = getBezierPath({
        sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition,
    });

    return (
        <>
            <defs>
                <marker id={`arrow-filled-${id}`} markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
                    <polygon points="0 0, 8 3, 0 6" fill={edgeStyle.color} />
                </marker>
                <marker id={`arrow-open-${id}`} markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
                    <path d="M0,0 L8,3 L0,6" fill="none" stroke={edgeStyle.color} strokeWidth="1.5" />
                </marker>
                <marker id={`dot-${id}`} markerWidth="8" markerHeight="8" refX="4" refY="4" orient="auto">
                    <circle cx="4" cy="4" r="3" fill="none" stroke={edgeStyle.color} strokeWidth="1.5" />
                </marker>
            </defs>
            <BaseEdge
                id={id}
                path={edgePath}
                style={{
                    ...style,
                    stroke: edgeStyle.color,
                    strokeWidth: edgeStyle.strokeWidth,
                    strokeDasharray: edgeStyle.strokeDasharray,
                }}
                markerEnd={edgeStyle.markerEnd ? `url(#${edgeStyle.markerEnd}-${id})` : undefined}
            />
        </>
    );
}

export const TypedEdge = memo(TypedEdgeComponent);
export default TypedEdge;
