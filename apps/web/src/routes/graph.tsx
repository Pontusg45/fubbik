import { useQuery } from "@tanstack/react-query";
import { createFileRoute, redirect, useNavigate } from "@tanstack/react-router";
import {
    Background,
    Controls,
    ReactFlow,
    useEdgesState,
    useNodesState,
    type Edge,
    type Node
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import Dagre from "@dagrejs/dagre";
import { useEffect, useMemo } from "react";

import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/graph")({
    component: GraphView,
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            throw redirect({ to: "/login" });
        }
    }
});

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

        const rawNodes: Node[] = chunks.map(c => ({
            id: c.id,
            data: { label: c.title },
            position: { x: 0, y: 0 },
            style: { cursor: "pointer" }
        }));

        const rawEdges: Edge[] = connections.map(conn => ({
            id: conn.id,
            source: conn.sourceId,
            target: conn.targetId,
            label: conn.relation,
            animated: true
        }));

        for (const node of rawNodes) {
            g.setNode(node.id, { width: 200, height: 50 });
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
