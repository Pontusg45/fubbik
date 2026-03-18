import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
    Background,
    Controls,
    ReactFlow,
    ReactFlowProvider,
    type Edge,
    type Node
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import dagre from "@dagrejs/dagre";
import { useState, useCallback, useMemo } from "react";

import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface DependencyGraphProps {
    requirementId: string;
}

interface GraphNode {
    id: string;
    title: string;
    status: string;
    priority: string | null;
    isCurrent: boolean;
}

interface GraphEdge {
    source: string;
    target: string;
}

const statusColors: Record<string, { bg: string; border: string }> = {
    passing: { bg: "#dcfce7", border: "#22c55e" },
    failing: { bg: "#fee2e2", border: "#ef4444" },
    untested: { bg: "#f3f4f6", border: "#9ca3af" }
};

function layoutGraph(nodes: Node[], edges: Edge[]): Node[] {
    const g = new dagre.graphlib.Graph();
    g.setDefaultEdgeLabel(() => ({}));
    g.setGraph({ rankdir: "TB", nodesep: 50, ranksep: 80 });
    nodes.forEach(n => g.setNode(n.id, { width: 200, height: 60 }));
    edges.forEach(e => g.setEdge(e.source, e.target));
    dagre.layout(g);
    return nodes.map(n => {
        const pos = g.node(n.id);
        return { ...n, position: { x: pos.x - 100, y: pos.y - 30 } };
    });
}

function DependencyGraphInner({ requirementId }: DependencyGraphProps) {
    const navigate = useNavigate();

    const { data, isLoading } = useQuery({
        queryKey: ["dependency-graph", requirementId],
        queryFn: async () => {
            return unwrapEden(
                await api.api.requirements({ id: requirementId }).dependencies.graph.get()
            ) as { nodes: GraphNode[]; edges: GraphEdge[] };
        }
    });

    const onNodeClick = useCallback(
        (_: React.MouseEvent, node: Node) => {
            navigate({ to: "/requirements/$requirementId", params: { requirementId: node.id } });
        },
        [navigate]
    );

    const { nodes, edges } = useMemo(() => {
        if (!data) return { nodes: [], edges: [] };

        const flowEdges: Edge[] = data.edges.map(e => ({
            id: `${e.source}-${e.target}`,
            source: e.source,
            target: e.target,
            type: "default",
            animated: false,
            markerEnd: { type: "arrowclosed" as const }
        }));

        const flowNodes: Node[] = data.nodes.map(n => {
            const colors = statusColors[n.status] ?? statusColors.untested!;
            return {
                id: n.id,
                type: "default",
                position: { x: 0, y: 0 },
                data: { label: n.title },
                style: {
                    background: colors!.bg,
                    borderColor: colors!.border,
                    borderWidth: n.isCurrent ? 3 : 1,
                    borderStyle: "solid",
                    borderRadius: 8,
                    padding: "8px 12px",
                    fontSize: 13,
                    width: 200
                }
            };
        });

        const laid = layoutGraph(flowNodes, flowEdges);
        return { nodes: laid, edges: flowEdges };
    }, [data]);

    if (isLoading) {
        return <p className="text-muted-foreground py-4 text-center text-sm">Loading graph...</p>;
    }

    if (!data || data.nodes.length === 0) {
        return null;
    }

    return (
        <div className="h-[400px] rounded-lg border">
            <ReactFlow
                nodes={nodes}
                edges={edges}
                onNodeClick={onNodeClick}
                fitView
                proOptions={{ hideAttribution: true }}
                nodesDraggable={false}
                nodesConnectable={false}
            >
                <Background />
                <Controls showInteractive={false} />
            </ReactFlow>
        </div>
    );
}

export function DependencyGraph({ requirementId }: DependencyGraphProps) {
    const [expanded, setExpanded] = useState(false);

    return (
        <div className="mb-6">
            <Button variant="outline" size="sm" onClick={() => setExpanded(!expanded)} className="mb-3">
                {expanded ? "Hide dependency graph" : "View dependency graph"}
            </Button>
            {expanded && (
                <ReactFlowProvider>
                    <DependencyGraphInner requirementId={requirementId} />
                </ReactFlowProvider>
            )}
        </div>
    );
}
