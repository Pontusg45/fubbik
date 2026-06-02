import { Link } from "@tanstack/react-router";
import { ArrowRight } from "lucide-react";

interface PathNode {
    id: string;
    title: string;
    type: string;
}

const TYPE_COLORS: Record<string, string> = {
    note: "bg-blue-500/10 text-blue-400 border-blue-500/20",
    document: "bg-purple-500/10 text-purple-400 border-purple-500/20",
    reference: "bg-green-500/10 text-green-400 border-green-500/20",
    schema: "bg-orange-500/10 text-orange-400 border-orange-500/20",
    checklist: "bg-pink-500/10 text-pink-400 border-pink-500/20"
};

export function PathView({ nodes }: { nodes: PathNode[] }) {
    if (nodes.length === 0) return null;

    return (
        <div className="overflow-x-auto rounded-lg border border-indigo-500/20 bg-indigo-500/5 p-4">
            <div className="flex min-w-max items-center gap-0">
                {nodes.map((node, i) => {
                    const typeColor = TYPE_COLORS[node.type] ?? "bg-slate-500/10 text-slate-400 border-slate-500/20";
                    const isLast = i === nodes.length - 1;

                    return (
                        <div key={node.id} className="flex items-center">
                            {/* Node card */}
                            <Link
                                to="/chunks/$chunkId"
                                params={{ chunkId: node.id }}
                                className="bg-card hover:bg-muted/50 flex w-[130px] flex-col items-center gap-1.5 rounded-lg border px-3 py-2.5 transition-colors"
                            >
                                <span className="w-full truncate text-center text-sm leading-tight font-medium">{node.title}</span>
                                <span className={`rounded border px-1.5 py-0.5 font-mono text-[9px] font-medium ${typeColor}`}>
                                    {node.type}
                                </span>
                            </Link>

                            {/* Arrow connector */}
                            {!isLast && (
                                <div className="flex items-center px-2">
                                    <ArrowRight className="text-muted-foreground/40 size-4" />
                                </div>
                            )}
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
