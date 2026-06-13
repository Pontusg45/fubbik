import { Handle, Position, type NodeProps } from "@xyflow/react";
import { memo } from "react";

interface ConceptNodeData {
    label: string;
    strength: number;
    memberCount: number;
    [key: string]: unknown;
}

function ConceptNodeComponent({ data }: NodeProps) {
    const d = data as ConceptNodeData;
    const opacity = Math.min(1, 0.3 + (d.strength / 20) * 0.7);
    return (
        <div
            className="rounded-full border-2 border-dashed border-violet-400 bg-violet-50 px-3 py-1.5 text-xs dark:border-violet-600 dark:bg-violet-950"
            style={{ opacity }}
        >
            <Handle type="target" position={Position.Top} className="!bg-violet-400" />
            <div className="flex items-center gap-1.5">
                <span className="font-semibold text-violet-700 dark:text-violet-300">{d.label}</span>
                <span className="text-[10px] text-violet-500">{d.memberCount}</span>
            </div>
            <Handle type="source" position={Position.Bottom} className="!bg-violet-400" />
        </div>
    );
}

export const ConceptNode = memo(ConceptNodeComponent);
export default ConceptNode;
