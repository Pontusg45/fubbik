import Dagre from "@dagrejs/dagre";
import { useQuery } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import "@xyflow/react/dist/style.css";
import { Background, Controls, ReactFlow, useEdgesState, useNodesState, type Edge, type Node } from "@xyflow/react";
import { useEffect, useMemo } from "react";

import { relationColor } from "@/features/chunks/relation-colors";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/graph")({
    component: GraphView,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest access
        }
        return { session };
    }
});

const TYPE_COLORS: Record<string, { bg: string; border: string }> = {
    note: { bg: "#1e293b", border: "#475569" },
    guide: { bg: "#1e1b4b", border: "#6366f1" },
    reference: { bg: "#042f2e", border: "#14b8a6" },
    document: { bg: "#172554", border: "#3b82f6" },
    schema: { bg: "#1c1917", border: "#f59e0b" },
    checklist: { bg: "#1a2e05", border: "#84cc16" }
};

function GraphView() {
    const navigate = useNavigate();

    const { data, isLoading } = useQuery({
        queryKey: ["graph"],
        queryFn: async () => {
            return unwrapEden(await api.api.graph.get());
        }
    });

    const { layoutNodes, layoutEdges } = useMemo(() => {
        const chunks = data?.chunks ?? [];
        const connections = data?.connections ?? [];

        if (chunks.length === 0) return { layoutNodes: [] as Node[], layoutEdges: [] as Edge[] };

        const g = new Dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
        g.setGraph({ rankdir: "TB", nodesep: 80, ranksep: 100 });

        const MAIN_NODE_ID = "__main__";

        const rawNodes: Node[] = [
            {
                id: MAIN_NODE_ID,
                data: { label: "Knowledge Base" },
                position: { x: 0, y: 0 },
                style: {
                    cursor: "default",
                    background: "#0f172a",
                    borderColor: "#e2e8f0",
                    borderWidth: 2,
                    borderRadius: 12,
                    color: "#f8fafc",
                    fontSize: 14,
                    fontWeight: 600,
                    padding: "10px 16px"
                }
            },
            ...chunks.map(c => {
                const typeColor = TYPE_COLORS[c.type] ?? TYPE_COLORS.note;
                return {
                    id: c.id,
                    data: { label: c.title },
                    position: { x: 0, y: 0 },
                    style: {
                        cursor: "pointer",
                        background: typeColor!.bg,
                        borderColor: typeColor!.border,
                        borderWidth: 1.5,
                        borderRadius: 8,
                        color: "#e2e8f0",
                        fontSize: 12,
                        padding: "8px 12px"
                    }
                };
            })
        ];

        const rawEdges: Edge[] = connections.map(conn => {
            const color = relationColor(conn.relation);
            return {
                id: conn.id,
                source: conn.sourceId,
                target: conn.targetId,
                label: conn.relation,
                animated: true,
                style: { stroke: color, strokeWidth: 2 },
                labelStyle: { fill: color, fontSize: 10, fontWeight: 500 },
                labelBgStyle: { fill: "rgba(0,0,0,0.6)", fillOpacity: 0.8 }
            };
        });

        // Find chunks with no connections and link them to the main node
        const connectedIds = new Set<string>();
        for (const conn of connections) {
            connectedIds.add(conn.sourceId);
            connectedIds.add(conn.targetId);
        }
        for (const c of chunks) {
            if (!connectedIds.has(c.id)) {
                rawEdges.push({
                    id: `main-${c.id}`,
                    source: MAIN_NODE_ID,
                    target: c.id,
                    animated: false,
                    style: { stroke: "#334155", strokeWidth: 1, strokeDasharray: "4 4" }
                });
            }
        }

        for (const node of rawNodes) {
            g.setNode(node.id, { width: node.id === MAIN_NODE_ID ? 160 : 200, height: 50 });
        }
        for (const edge of rawEdges) {
            g.setEdge(edge.source, edge.target);
        }

        Dagre.layout(g);

        const layoutNodes = rawNodes.map(node => {
            const pos = g.node(node.id);
            return { ...node, position: { x: pos.x - 100, y: pos.y - 25 } };
        });

        return { layoutNodes, layoutEdges: rawEdges };
    }, [data]);

    const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
    const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);

    useEffect(() => {
        setNodes(layoutNodes);
    }, [layoutNodes, setNodes]);

    useEffect(() => {
        setEdges(layoutEdges);
    }, [layoutEdges, setEdges]);

    if (isLoading) {
        return (
            <div className="flex h-[calc(100vh-4rem)] items-center justify-center">
                <p className="text-muted-foreground">Loading graph...</p>
            </div>
        );
    }

    return (
        <div className="h-[calc(100vh-4rem)]">
            <ReactFlow
                nodes={nodes}
                edges={edges}
                onNodesChange={onNodesChange}
                onEdgesChange={onEdgesChange}
                onNodeClick={(_, node) => {
                    if (node.id === "__main__") return;
                    navigate({ to: "/chunks/$chunkId", params: { chunkId: node.id } });
                }}
                fitView
                colorMode="dark"
            >
                <Background />
                <Controls />
            </ReactFlow>
        </div>
    );
}
