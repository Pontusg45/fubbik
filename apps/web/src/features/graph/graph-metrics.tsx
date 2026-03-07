import type { Edge, Node } from "@xyflow/react";
import { BarChart3, GitFork, Unlink } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";

const MAIN_NODE_ID = "__main__";

interface GraphMetricsProps {
    nodes: Node[];
    edges: Edge[];
    onNodeClick: (id: string) => void;
}

export function GraphMetrics({ nodes, edges, onNodeClick }: GraphMetricsProps) {
    const [expanded, setExpanded] = useState(false);

    const metrics = useMemo(() => {
        const chunkNodes = nodes.filter(n => n.id !== MAIN_NODE_ID);
        const realEdges = edges.filter(e => !e.id.startsWith("main-"));

        const counts = new Map<string, number>();
        for (const edge of realEdges) {
            counts.set(edge.source, (counts.get(edge.source) ?? 0) + 1);
            counts.set(edge.target, (counts.get(edge.target) ?? 0) + 1);
        }

        const hubs = [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 5);
        const isolated = chunkNodes.filter(n => !counts.has(n.id));

        const typeCounts = new Map<string, number>();
        for (const node of chunkNodes) {
            const t = (node.data as { type?: string })?.type ?? "unknown";
            typeCounts.set(t, (typeCounts.get(t) ?? 0) + 1);
        }

        const totalConnections = [...counts.values()].reduce((a, b) => a + b, 0);
        const avgConnections = chunkNodes.length > 0 ? totalConnections / chunkNodes.length : 0;
        const maxEdges = (chunkNodes.length * (chunkNodes.length - 1)) / 2;
        const density = maxEdges > 0 ? realEdges.length / maxEdges : 0;

        return {
            hubs,
            isolated,
            typeCounts,
            avgConnections,
            density,
            totalNodes: chunkNodes.length,
            totalEdges: realEdges.length
        };
    }, [nodes, edges]);

    return (
        <div className="bg-background/90 absolute bottom-4 left-4 z-10 max-w-[240px] rounded-lg border backdrop-blur-sm">
            <button
                onClick={() => setExpanded(!expanded)}
                className="text-muted-foreground hover:text-foreground flex w-full items-center gap-2 px-3 py-2 text-xs font-medium"
                type="button"
            >
                <BarChart3 className="size-3.5" />
                Metrics
                <span className="ml-auto text-[10px]">{expanded ? "\u2212" : "+"}</span>
            </button>

            {expanded && (
                <div className="space-y-3 border-t px-3 py-2">
                    {/* Overview grid */}
                    <div className="grid grid-cols-2 gap-2 text-xs">
                        <div>
                            <p className="text-muted-foreground text-[10px]">Nodes</p>
                            <p className="font-medium">{metrics.totalNodes}</p>
                        </div>
                        <div>
                            <p className="text-muted-foreground text-[10px]">Edges</p>
                            <p className="font-medium">{metrics.totalEdges}</p>
                        </div>
                        <div>
                            <p className="text-muted-foreground text-[10px]">Avg connections</p>
                            <p className="font-medium">{metrics.avgConnections.toFixed(1)}</p>
                        </div>
                        <div>
                            <p className="text-muted-foreground text-[10px]">Density</p>
                            <p className="font-medium">{(metrics.density * 100).toFixed(1)}%</p>
                        </div>
                    </div>

                    {/* Hubs */}
                    {metrics.hubs.length > 0 && (
                        <div>
                            <p className="text-muted-foreground mb-1 flex items-center gap-1 text-[10px] font-medium uppercase">
                                <GitFork className="size-2.5" /> Top hubs
                            </p>
                            <div className="space-y-0.5">
                                {metrics.hubs.map(([id, count]) => {
                                    const node = nodes.find(n => n.id === id);
                                    const label = typeof node?.data?.label === "string" ? node.data.label : id.slice(0, 8);
                                    return (
                                        <button
                                            key={id}
                                            onClick={() => onNodeClick(id)}
                                            className="hover:bg-muted flex w-full items-center justify-between rounded px-1.5 py-0.5 text-[11px]"
                                            type="button"
                                        >
                                            <span className="truncate">{label}</span>
                                            <Badge variant="outline" size="sm" className="text-[9px]">
                                                {count}
                                            </Badge>
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    {/* Isolated */}
                    {metrics.isolated.length > 0 && (
                        <div>
                            <p className="text-muted-foreground mb-1 flex items-center gap-1 text-[10px] font-medium uppercase">
                                <Unlink className="size-2.5" /> Isolated ({metrics.isolated.length})
                            </p>
                            <div className="max-h-24 space-y-0.5 overflow-y-auto">
                                {metrics.isolated.slice(0, 10).map(node => {
                                    const label = typeof node.data?.label === "string" ? node.data.label : node.id.slice(0, 8);
                                    return (
                                        <button
                                            key={node.id}
                                            onClick={() => onNodeClick(node.id)}
                                            className="hover:bg-muted block w-full truncate rounded px-1.5 py-0.5 text-left text-[11px]"
                                            type="button"
                                        >
                                            {label}
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    {/* Types */}
                    <div>
                        <p className="text-muted-foreground mb-1 text-[10px] font-medium uppercase">By type</p>
                        <div className="flex flex-wrap gap-1">
                            {[...metrics.typeCounts.entries()].map(([type, count]) => (
                                <Badge key={type} variant="secondary" size="sm" className="text-[9px]">
                                    {type}: {count}
                                </Badge>
                            ))}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
