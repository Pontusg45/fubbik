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
                style={{ background: `${color}15`, border: `1.5px dashed ${color}30` }}
            >
                <Handle type="source" position={Position.Top} className="!invisible" />
                <Handle type="target" position={Position.Bottom} className="!invisible" />
                <div className="text-[11px] font-medium" style={{ color }}>{name}</div>
                <div className="mt-0.5 text-[9px] text-slate-500">1 chunk</div>
            </div>
        );
    }

    const MAX_DOTS = 20;
    const displayDots = healthScores.slice(0, MAX_DOTS);
    const overflow = healthScores.length - MAX_DOTS;

    return (
        <div
            className="relative max-w-[200px] min-w-[120px] rounded-2xl backdrop-blur-sm"
            style={{
                background: `${color}0a`,
                border: `2px solid ${color}30`,
                boxShadow: `0 0 30px ${color}08, inset 0 0 30px ${color}05`,
            }}
        >
            <Handle type="source" position={Position.Top} className="!invisible" />
            <Handle type="target" position={Position.Bottom} className="!invisible" />
            <Handle type="source" position={Position.Left} className="!invisible" id="left" />
            <Handle type="target" position={Position.Right} className="!invisible" id="right" />

            {/* Group label badge */}
            <div
                className="absolute -top-2.5 left-1/2 -translate-x-1/2 rounded-full px-2.5 py-0.5 text-[9px] font-bold uppercase tracking-wider whitespace-nowrap"
                style={{ background: `${color}25`, color, border: `1px solid ${color}40` }}
            >
                {name}
            </div>

            {/* Body */}
            <div className="px-4 pt-5 pb-3.5 text-center">
                <div className="text-[22px] font-semibold leading-none" style={{ color }}>
                    {chunkCount}
                </div>
                <div className="mt-0.5 text-[9px] text-slate-500">chunks</div>

                {displayDots.length > 0 && (
                    <div className="mt-2.5 flex flex-wrap justify-center gap-[3px]">
                        {displayDots.map((score, i) => (
                            <div
                                key={i}
                                className="h-[6px] w-[6px] rounded-full"
                                style={{ background: healthColor(score) }}
                            />
                        ))}
                        {overflow > 0 && (
                            <span className="ml-0.5 text-[8px] text-slate-500">+{overflow}</span>
                        )}
                    </div>
                )}
            </div>
        </div>
    );
}

export const GraphIslandNode = memo(IslandNodeComponent);
export default GraphIslandNode;
