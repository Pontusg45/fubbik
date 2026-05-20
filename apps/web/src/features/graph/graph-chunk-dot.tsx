import { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";

export interface ChunkDotData {
    title: string;
    chunkType: string;
    healthScore: number;
    [key: string]: unknown;
}

const TYPE_ACCENT: Record<string, string> = {
    note: "#94a3b8",
    guide: "#6366f1",
    reference: "#14b8a6",
    document: "#3b82f6",
    schema: "#f59e0b",
    checklist: "#84cc16",
};

function healthColor(score: number): string {
    if (score >= 80) return "#22c55e";
    if (score >= 60) return "#4ade80";
    if (score >= 40) return "#f59e0b";
    return "#ef4444";
}

function ChunkDotComponent({ data }: NodeProps) {
    const { title, chunkType, healthScore } = data as ChunkDotData;
    const accent = TYPE_ACCENT[chunkType] ?? "#64748b";

    return (
        <div className="flex items-center gap-1.5">
            <Handle type="source" position={Position.Top} className="!invisible" />
            <Handle type="target" position={Position.Bottom} className="!invisible" />
            <div className="relative flex h-4 w-4 shrink-0 items-center justify-center">
                <div
                    className="absolute inset-0 rounded-full opacity-20"
                    style={{ background: accent }}
                />
                <div
                    className="h-2 w-2 rounded-full"
                    style={{ background: healthColor(healthScore) }}
                />
            </div>
            <span
                className="max-w-[90px] truncate text-[9px] leading-tight"
                style={{ color: accent }}
            >
                {title}
            </span>
        </div>
    );
}

export const GraphChunkDot = memo(ChunkDotComponent);
export default GraphChunkDot;
