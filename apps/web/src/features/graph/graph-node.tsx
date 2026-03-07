import { Handle, Position, type NodeProps } from "@xyflow/react";
import { BookOpen, CheckSquare, Compass, Database, FileText, StickyNote } from "lucide-react";
import { useContext } from "react";
import { ZoomContext } from "./graph-view";

const TYPE_ICONS: Record<string, typeof BookOpen> = {
    guide: BookOpen,
    document: FileText,
    checklist: CheckSquare,
    schema: Database,
    reference: Compass,
    note: StickyNote
};

export function GraphNode({ data }: NodeProps) {
    const nodeData = data as { label: string; type?: string; connectionCount?: number; scale?: number; tags?: string[] };
    const Icon = TYPE_ICONS[nodeData.type ?? ""] ?? StickyNote;
    const count = nodeData.connectionCount ?? 0;
    const scale = nodeData.scale ?? 1;
    const zoomTier = useContext(ZoomContext);
    const tags = nodeData.tags ?? [];

    // Compact mode at low zoom
    if (zoomTier === "compact") {
        return (
            <>
                <Handle type="target" position={Position.Top} className="!bg-muted-foreground/50 !border-muted-foreground/50 !size-2" />
                <Handle type="target" position={Position.Left} className="!bg-muted-foreground/50 !border-muted-foreground/50 !size-2" id="left-target" />
                <Handle type="source" position={Position.Right} className="!bg-muted-foreground/50 !border-muted-foreground/50 !size-2" id="right-source" />
                <div className="flex items-center gap-0.5">
                    <Icon className="size-2.5 shrink-0 opacity-60" />
                    <span className="text-[9px] leading-tight">{nodeData.label}</span>
                </div>
                <Handle type="source" position={Position.Bottom} className="!bg-muted-foreground/50 !border-muted-foreground/50 !size-2" />
            </>
        );
    }

    // Normal + high zoom mode
    return (
        <>
            <Handle type="target" position={Position.Top} className="!bg-muted-foreground/50 !border-muted-foreground/50 !size-2" />
            <Handle type="target" position={Position.Left} className="!bg-muted-foreground/50 !border-muted-foreground/50 !size-2" id="left-target" />
            <Handle type="source" position={Position.Right} className="!bg-muted-foreground/50 !border-muted-foreground/50 !size-2" id="right-source" />
            <div className="flex flex-col gap-0.5">
                <div className="flex items-center gap-1.5">
                    <Icon className={`${scale > 1.3 ? "size-3.5" : "size-3"} shrink-0 opacity-60`} />
                    <span className="leading-tight">{nodeData.label}</span>
                    {count > 0 && (
                        <span className="flex size-3.5 items-center justify-center rounded-full bg-current/10 text-[8px] font-medium leading-none opacity-60">
                            {count}
                        </span>
                    )}
                </div>
                {zoomTier === "detailed" && tags.length > 0 && (
                    <div className="flex flex-wrap gap-0.5">
                        {tags.slice(0, 3).map(tag => (
                            <span key={tag} className="rounded-full bg-current/10 px-1 py-px text-[8px] opacity-60">
                                {tag}
                            </span>
                        ))}
                    </div>
                )}
            </div>
            <Handle type="source" position={Position.Bottom} className="!bg-muted-foreground/50 !border-muted-foreground/50 !size-2" />
        </>
    );
}
