import { Handle, Position, type NodeProps } from "@xyflow/react";
import { memo } from "react";

export interface ChunkCardData {
    title: string;
    summary?: string | null;
    type: string;
    tags: Array<{ name: string; color: string }>;
    healthScore: number;
    isFocus: boolean;
    impactDegree?: number; // 0-1, from upstream_impact staleness flags
    [key: string]: unknown;
}

function healthColor(score: number): string {
    if (score >= 80) return "#22c55e";
    if (score >= 60) return "#4ade80";
    if (score >= 40) return "#f59e0b";
    return "#ef4444";
}

function ChunkCardComponent({ data }: NodeProps) {
    const { title, summary, tags, healthScore, isFocus, impactDegree } = data as ChunkCardData;

    return (
        <div
            className="relative max-w-[160px] rounded-[10px] border bg-slate-800 text-left"
            style={{
                borderWidth: isFocus ? "2px" : "1px",
                borderColor: isFocus ? "#3b82f6" : "#334155",
                padding: isFocus ? "10px 14px" : "7px 11px",
                boxShadow: isFocus ? "0 0 24px rgba(59,130,246,0.25)" : "none"
            }}
        >
            <Handle type="source" position={Position.Top} className="!invisible" />
            <Handle type="target" position={Position.Bottom} className="!invisible" />
            <Handle type="source" position={Position.Left} className="!invisible" id="left" />
            <Handle type="target" position={Position.Right} className="!invisible" id="right" />
            <div className="flex items-center gap-1.5">
                <div className="h-2 w-2 flex-shrink-0 rounded-full" style={{ background: healthColor(healthScore) }} />
                <div className="truncate font-medium text-slate-200" style={{ fontSize: isFocus ? "11px" : "10px" }}>
                    {title}
                </div>
            </div>
            {summary && (
                <div className="mt-1 line-clamp-2 text-slate-500" style={{ fontSize: isFocus ? "9px" : "8px", lineHeight: "1.3" }}>
                    {summary}
                </div>
            )}
            {isFocus && tags.length > 0 && (
                <div className="mt-1.5 flex flex-wrap gap-1">
                    {tags.slice(0, 3).map(tag => (
                        <span
                            key={tag.name}
                            className="rounded-[3px] px-1 py-px text-[7px]"
                            style={{ background: `${tag.color}20`, color: tag.color }}
                        >
                            {tag.name}
                        </span>
                    ))}
                </div>
            )}
            {impactDegree != null && impactDegree > 0 && (
                <div
                    className="pointer-events-none absolute inset-0 rounded-lg border-2 border-amber-400"
                    style={{ opacity: impactDegree }}
                />
            )}
        </div>
    );
}

export const GraphChunkCard = memo(ChunkCardComponent);
export default GraphChunkCard;
