import { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";

export interface ChunkDotData {
    title: string;
    color: string;
    [key: string]: unknown;
}

function ChunkDotComponent({ data }: NodeProps) {
    const { title, color } = data as ChunkDotData;
    return (
        <div className="group flex flex-col items-center">
            <Handle type="source" position={Position.Top} className="!invisible" />
            <Handle type="target" position={Position.Bottom} className="!invisible" />
            <div className="h-[8px] w-[8px] rounded-full opacity-70" style={{ background: color }} />
            <div className="mt-0.5 max-w-[80px] truncate text-center text-[7px] text-slate-500 opacity-0 transition-opacity group-hover:opacity-100">
                {title}
            </div>
        </div>
    );
}

export const GraphChunkDot = memo(ChunkDotComponent);
export default GraphChunkDot;
