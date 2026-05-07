import type { NodeProps } from "@xyflow/react";
import { memo } from "react";

interface ClusterNodeData {
    label: string;
    count: number;
    color: string;
    expanded: boolean;
    onToggle: () => void;
    [key: string]: unknown;
}

function hexToRgb(hex: string): { r: number; g: number; b: number } {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result
        ? { r: parseInt(result[1]!, 16), g: parseInt(result[2]!, 16), b: parseInt(result[3]!, 16) }
        : { r: 139, g: 92, b: 246 };
}

function GraphClusterNodeInner({ data }: NodeProps) {
    const { label, count, color, expanded, onToggle } = data as ClusterNodeData;
    const { r, g, b } = hexToRgb(color);

    return (
        <button
            type="button"
            onClick={onToggle}
            className="flex flex-col items-center justify-center gap-1 rounded-2xl px-6 py-4 shadow-md transition-transform hover:scale-105"
            style={{
                backgroundColor: `rgba(${r}, ${g}, ${b}, 0.15)`,
                border: `2.5px solid rgba(${r}, ${g}, ${b}, 0.7)`,
                minWidth: 160,
                minHeight: 80,
                cursor: "pointer",
            }}
        >
            <span
                className="text-xs font-semibold uppercase tracking-wider"
                style={{ color: `rgba(${r}, ${g}, ${b}, 0.9)` }}
            >
                {label}
            </span>
            <span
                className="text-lg font-bold"
                style={{ color: `rgba(${r}, ${g}, ${b}, 1)` }}
            >
                {count} chunks
            </span>
            <span
                className="text-[10px]"
                style={{ color: `rgba(${r}, ${g}, ${b}, 0.7)` }}
            >
                {expanded ? "click to collapse" : "click to expand"}
            </span>
        </button>
    );
}

export const GraphClusterNode = memo(GraphClusterNodeInner, (prev, next) => {
    const a = prev.data as ClusterNodeData;
    const b = next.data as ClusterNodeData;
    return a === b || (a.label === b.label && a.count === b.count && a.color === b.color && a.expanded === b.expanded);
});
