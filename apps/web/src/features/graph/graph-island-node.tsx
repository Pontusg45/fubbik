import { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";

export interface IslandNodeData {
    name: string;
    chunkCount: number;
    color: string;
    healthScores: number[];
    isSingleton: boolean;
    [key: string]: unknown;
}

function healthColor(score: number): string {
    if (score >= 80) return "#22c55e";
    if (score >= 60) return "#4ade80";
    if (score >= 40) return "#f59e0b";
    return "#ef4444";
}

function IslandNodeComponent({ data }: NodeProps) {
    const { name, chunkCount, color, healthScores, isSingleton } = data as IslandNodeData;

    if (isSingleton) {
        return (
            <div
                className="rounded-lg px-3 py-2 text-center backdrop-blur-sm"
                style={{ background: `${color}15`, border: `1px solid ${color}30` }}
            >
                <Handle type="source" position={Position.Top} className="!invisible" />
                <Handle type="target" position={Position.Bottom} className="!invisible" />
                <div className="text-[11px] font-medium" style={{ color }}>{name}</div>
                <div className="mt-1 text-[9px] text-slate-500">1 chunk</div>
            </div>
        );
    }

    const MAX_DOTS = 20;
    const displayDots = healthScores.slice(0, MAX_DOTS);
    const overflow = healthScores.length - MAX_DOTS;

    return (
        <div
            className="max-w-[180px] min-w-[100px] rounded-3xl px-5 py-4 text-center backdrop-blur-sm"
            style={{ background: `${color}12`, border: `1.5px solid ${color}25` }}
        >
            <Handle type="source" position={Position.Top} className="!invisible" />
            <Handle type="target" position={Position.Bottom} className="!invisible" />
            <Handle type="source" position={Position.Left} className="!invisible" id="left" />
            <Handle type="target" position={Position.Right} className="!invisible" id="right" />
            <div className="truncate text-[11px] font-semibold" style={{ color }}>{name}</div>
            <div className="mt-1 text-[9px] text-slate-500">{chunkCount} chunks</div>
            {displayDots.length > 0 && (
                <div className="mt-2 flex flex-wrap justify-center gap-[2px]">
                    {displayDots.map((score, i) => (
                        <div key={i} className="h-[5px] w-[5px] rounded-full" style={{ background: healthColor(score) }} />
                    ))}
                    {overflow > 0 && (
                        <span className="text-[7px] text-slate-500">+{overflow}</span>
                    )}
                </div>
            )}
        </div>
    );
}

export const GraphIslandNode = memo(IslandNodeComponent);
export default GraphIslandNode;
