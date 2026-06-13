import { Handle, Position, type NodeProps } from "@xyflow/react";
import { memo } from "react";

interface CodeNodeData {
    name: string;
    kind: "file" | "function" | "class" | "type" | "interface" | "variable";
    language?: string;
    symbolCount?: number;
    [key: string]: unknown;
}

const KIND_ICONS: Record<string, string> = {
    file: "F",
    function: "fn",
    class: "C",
    type: "T",
    interface: "I",
    variable: "x"
};

function CodeNodeComponent({ data }: NodeProps) {
    const d = data as CodeNodeData;
    return (
        <div className="rounded border border-blue-300 bg-blue-50 px-2 py-1 text-xs dark:border-blue-700 dark:bg-blue-950">
            <Handle type="target" position={Position.Top} className="!bg-blue-400" />
            <div className="flex items-center gap-1">
                <span className="font-mono text-blue-600 dark:text-blue-400">{KIND_ICONS[d.kind] ?? "?"}</span>
                <span className="truncate font-medium">{d.name}</span>
                {d.symbolCount != null && (
                    <span className="ml-auto rounded bg-blue-200 px-1 text-[10px] dark:bg-blue-800">{d.symbolCount}</span>
                )}
            </div>
            <Handle type="source" position={Position.Bottom} className="!bg-blue-400" />
        </div>
    );
}

export const CodeNode = memo(CodeNodeComponent);
export default CodeNode;
