import type { NodeProps } from "@xyflow/react";
import { memo } from "react";

interface GroupNodeData {
    label: string;
    color: string;
    [key: string]: unknown;
}

function hexToRgb(hex: string): { r: number; g: number; b: number } {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result
        ? { r: parseInt(result[1]!, 16), g: parseInt(result[2]!, 16), b: parseInt(result[3]!, 16) }
        : { r: 139, g: 92, b: 246 };
}

function GraphGroupNodeInner({ data }: NodeProps) {
    const { label, color } = data as GroupNodeData;
    const { r, g, b } = hexToRgb(color);
    return (
        <div
            className="relative h-full w-full"
            style={{
                backgroundColor: `rgba(${r}, ${g}, ${b}, 0.3)`,
                border: `2px dashed rgba(${r}, ${g}, ${b}, 0.6)`,
                borderRadius: 16
            }}
        >
            <span
                className="absolute top-2 left-3 text-[10px] font-semibold uppercase tracking-wider"
                style={{ color: `rgba(${r}, ${g}, ${b}, 0.8)` }}
            >
                {label}
            </span>
        </div>
    );
}

// Group nodes change only when their label or color change. Position/size are
// handled by React Flow outside the render path. Memoize to stop re-renders on
// every hover/select of an unrelated chunk.
export const GraphGroupNode = memo(GraphGroupNodeInner, (prev, next) => {
    const a = prev.data as GroupNodeData;
    const b = next.data as GroupNodeData;
    return a === b || (a.label === b.label && a.color === b.color);
});
