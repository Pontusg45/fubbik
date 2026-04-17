import { Handle, Position, type NodeProps } from "@xyflow/react";
import { memo } from "react";

import { resolveChunkTypeIcon, useChunkTypeMeta } from "@/features/vocabularies/use-vocabularies";

interface GraphNodeData {
    label: string;
    type?: string;
    connectionCount?: number;
    tags?: string[];
    codebaseName?: string;
}

function GraphNodeInner({ data }: NodeProps) {
    const nodeData = data as unknown as GraphNodeData;
    const meta = useChunkTypeMeta(nodeData.type);
    const Icon = resolveChunkTypeIcon(meta.icon);
    const count = nodeData.connectionCount ?? 0;
    const tags = nodeData.tags ?? [];
    const codebaseName = nodeData.codebaseName;

    return (
        <>
            <Handle type="target" position={Position.Top} className="!size-3 !bg-muted-foreground/50 hover:!bg-primary hover:!scale-125 !border-background !border-2 transition-all duration-150" />
            <Handle
                type="target"
                position={Position.Left}
                className="!size-3 !bg-muted-foreground/50 hover:!bg-primary hover:!scale-125 !border-background !border-2 transition-all duration-150"
                id="left-target"
            />
            <Handle
                type="source"
                position={Position.Right}
                className="!size-3 !bg-muted-foreground/50 hover:!bg-primary hover:!scale-125 !border-background !border-2 transition-all duration-150"
                id="right-source"
            />
            <div className="flex flex-col gap-0.5">
                <div className="flex items-center gap-1.5">
                    <Icon className="size-3 shrink-0 opacity-60" />
                    <span className="leading-tight">{nodeData.label}</span>
                    {count > 0 && (
                        <span className="flex size-3.5 items-center justify-center rounded-full bg-current/10 text-[8px] leading-none font-medium opacity-60">
                            {count}
                        </span>
                    )}
                </div>
                {codebaseName && (
                    <span className="rounded-full bg-current/10 px-1 py-px text-[7px] opacity-50 italic">
                        {codebaseName}
                    </span>
                )}
                {tags.length > 0 && (
                    <div className="flex flex-wrap gap-0.5">
                        {tags.slice(0, 3).map(tag => (
                            <span key={tag} className="rounded-full bg-current/10 px-1 py-px text-[8px] opacity-60">
                                {tag}
                            </span>
                        ))}
                    </div>
                )}
            </div>
            <Handle type="source" position={Position.Bottom} className="!size-3 !bg-muted-foreground/50 hover:!bg-primary hover:!scale-125 !border-background !border-2 transition-all duration-150" />
        </>
    );
}

// React Flow re-renders every node when any graph state changes. Memoizing with
// shallow data equality cuts reconciliation cost from O(nodes) to O(changed nodes)
// on every select/hover/filter tick. Position changes are handled by React Flow
// outside this render path so they don't need to go through equality.
export const GraphNode = memo(GraphNodeInner, (prev, next) => {
    const a = prev.data as unknown as GraphNodeData;
    const b = next.data as unknown as GraphNodeData;
    if (a === b) return true;
    if (a.label !== b.label) return false;
    if (a.type !== b.type) return false;
    if ((a.connectionCount ?? 0) !== (b.connectionCount ?? 0)) return false;
    if (a.codebaseName !== b.codebaseName) return false;
    const at = a.tags ?? [];
    const bt = b.tags ?? [];
    if (at.length !== bt.length) return false;
    for (let i = 0; i < at.length; i++) if (at[i] !== bt[i]) return false;
    return true;
});
