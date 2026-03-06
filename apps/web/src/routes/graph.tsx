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

    const initialNodes: Node[] = useMemo(() => {
        const chunks = data?.chunks ?? [];
        return chunks.map((c, i) => ({
            id: c.id,
            data: { label: c.title },
            position: { x: (i % 5) * 280, y: Math.floor(i / 5) * 160 },
            style: { cursor: "pointer" }
        }));
    }, [data]);

    const initialEdges: Edge[] = useMemo(() => {
        const connections = data?.connections ?? [];
        return connections.map(conn => ({
            id: conn.id,
            source: conn.sourceId,
            target: conn.targetId,
            label: conn.relation,
            animated: true
        }));
    }, [data]);

    const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
    const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);

    useEffect(() => {
        setNodes(initialNodes);
    }, [initialNodes, setNodes]);

    useEffect(() => {
        setEdges(initialEdges);
    }, [initialEdges, setEdges]);

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
