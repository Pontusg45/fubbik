import { Handle, Position, type NodeProps } from "@xyflow/react";
import { BookOpen, CheckSquare, Compass, Database, FileText, StickyNote } from "lucide-react";

const TYPE_ICONS: Record<string, typeof BookOpen> = {
    guide: BookOpen,
    document: FileText,
    checklist: CheckSquare,
    schema: Database,
    reference: Compass,
    note: StickyNote
};

export function GraphNode({ data }: NodeProps) {
    const nodeData = data as { label: string; type?: string; connectionCount?: number };
    const Icon = TYPE_ICONS[nodeData.type ?? ""] ?? StickyNote;
    const count = nodeData.connectionCount ?? 0;

    return (
        <>
            <Handle type="target" position={Position.Top} style={{ visibility: "hidden" }} />
            <div className="flex items-center gap-2">
                <Icon className="size-3.5 shrink-0 opacity-60" />
                <span>{nodeData.label}</span>
                {count > 0 && (
                    <span className="flex size-4 items-center justify-center rounded-full bg-white/10 text-[9px] font-medium leading-none opacity-60">
                        {count}
                    </span>
                )}
            </div>
            <Handle type="source" position={Position.Bottom} style={{ visibility: "hidden" }} />
        </>
    );
}
