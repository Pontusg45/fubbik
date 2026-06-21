import { Handle, Position, type NodeProps } from "@xyflow/react";
import { ShieldCheck } from "lucide-react";
import { memo } from "react";

interface BehaviorNodeData {
    title: string;
    layer?: string;
    category?: string;
    [key: string]: unknown;
}

const LAYER_LABELS: Record<string, string> = {
    invariant: "invariant",
    contract: "contract"
};

function BehaviorNodeComponent({ data }: NodeProps) {
    const d = data as BehaviorNodeData;
    const layerLabel = d.layer ? (LAYER_LABELS[d.layer] ?? d.layer) : null;
    return (
        <div className="max-w-[180px] rounded-md border-2 border-amber-400 bg-amber-50 px-2.5 py-1.5 text-xs shadow-sm dark:border-amber-600 dark:bg-amber-950">
            <Handle type="target" position={Position.Top} className="!bg-amber-400" />
            <div className="flex items-center gap-1.5">
                <ShieldCheck className="size-3.5 shrink-0 text-amber-600 dark:text-amber-400" />
                <span className="truncate font-semibold text-amber-900 dark:text-amber-100">{d.title}</span>
            </div>
            {layerLabel && (
                <span className="mt-1 inline-block rounded bg-amber-200 px-1.5 py-0.5 text-[10px] font-medium text-amber-800 dark:bg-amber-800 dark:text-amber-100">
                    {layerLabel}
                </span>
            )}
            <Handle type="source" position={Position.Bottom} className="!bg-amber-400" />
        </div>
    );
}

export const BehaviorNode = memo(BehaviorNodeComponent);
export default BehaviorNode;
