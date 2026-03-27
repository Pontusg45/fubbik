import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import "@xyflow/react/dist/style.css";
import {
    Background,
    BackgroundVariant,
    Controls,
    MiniMap,
    ReactFlow,
    ReactFlowProvider,
    useEdgesState,
    useNodesState,
    useReactFlow,
    type Edge,
    type Node
} from "@xyflow/react";
import { ArrowLeft, Edit2, Trash2 } from "lucide-react";
import { useTheme } from "next-themes";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { Spinner } from "@/components/ui/spinner";
import { relationColor } from "@/features/chunks/relation-colors";
import { FloatingEdge } from "@/features/graph/floating-edge";
import { GraphNode } from "@/features/graph/graph-node";
import { GraphDetailPanel } from "@/features/graph/graph-detail-panel";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

const EDGE_TYPES = { floating: FloatingEdge };
const NODE_TYPES = { chunk: GraphNode };

const TYPE_COLORS_DARK: Record<string, { bg: string; border: string }> = {
    note: { bg: "#1e293b", border: "#475569" },
    guide: { bg: "#1e1b4b", border: "#6366f1" },
    reference: { bg: "#042f2e", border: "#14b8a6" },
    document: { bg: "#172554", border: "#3b82f6" },
    schema: { bg: "#1c1917", border: "#f59e0b" },
    checklist: { bg: "#1a2e05", border: "#84cc16" }
};

const TYPE_COLORS_LIGHT: Record<string, { bg: string; border: string }> = {
    note: { bg: "#f1f5f9", border: "#94a3b8" },
    guide: { bg: "#eef2ff", border: "#6366f1" },
    reference: { bg: "#f0fdfa", border: "#14b8a6" },
    document: { bg: "#eff6ff", border: "#3b82f6" },
    schema: { bg: "#fefce8", border: "#f59e0b" },
    checklist: { bg: "#f7fee7", border: "#84cc16" }
};

function SavedGraphViewInner() {
    const { resolvedTheme } = useTheme();
    const isDark = resolvedTheme !== "light";
    const TYPE_COLORS = isDark ? TYPE_COLORS_DARK : TYPE_COLORS_LIGHT;
    const { graphId } = useParams({ strict: false }) as { graphId: string };
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const { fitView } = useReactFlow();
    const initialFitDoneRef = useRef(false);

    const [selectedChunkId, setSelectedChunkId] = useState<string | null>(null);
    const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
    const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);

    // Fetch saved graph
    const { data: savedGraph, isLoading: isLoadingSavedGraph } = useQuery({
        queryKey: ["saved-graphs", graphId],
        queryFn: async () => unwrapEden(await api.api["saved-graphs"]({ id: graphId }).get()),
        enabled: !!graphId
    });

    // Fetch full graph data (we filter to only saved chunk IDs)
    const { data: graphData, isLoading: isLoadingGraph } = useQuery({
        queryKey: ["graph"],
        queryFn: async () => unwrapEden(await api.api.graph.get({ query: {} }))
    });

    const deleteMutation = useMutation({
        mutationFn: async () => unwrapEden(await api.api["saved-graphs"]({ id: graphId }).delete()),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["saved-graphs"] });
            toast.success("Saved graph deleted");
            navigate({ to: "/graph" });
        }
    });

    // Update positions mutation (for saving repositioned nodes)
    const updatePositionsMutation = useMutation({
        mutationFn: async (positions: Record<string, { x: number; y: number }>) => {
            return unwrapEden(
                await api.api["saved-graphs"]({ id: graphId }).patch({ positions })
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["saved-graphs", graphId] });
            toast.success("Positions saved");
        }
    });

    // Build nodes and edges from saved graph + graph data
    const { graphNodes, graphEdges } = useMemo(() => {
        if (!savedGraph || !graphData) return { graphNodes: [] as Node[], graphEdges: [] as Edge[] };

        const sg = savedGraph as {
            chunkIds: string[];
            positions: Record<string, { x: number; y: number }>;
        };
        const savedChunkIds = new Set(sg.chunkIds);
        const positions = sg.positions;

        // Filter chunks to only those in the saved graph
        const chunks = (graphData as { chunks: Array<{ id: string; title: string; type: string }> }).chunks
            .filter(c => savedChunkIds.has(c.id));

        // Filter connections to only those between saved chunks
        const connections = (graphData as { connections: Array<{ id: string; sourceId: string; targetId: string; relation: string }> }).connections
            .filter(c => savedChunkIds.has(c.sourceId) && savedChunkIds.has(c.targetId));

        const graphNodes: Node[] = chunks.map(c => {
            const typeColor = TYPE_COLORS[c.type] ?? TYPE_COLORS.note;
            const pos = positions[c.id] ?? { x: 0, y: 0 };
            return {
                id: c.id,
                type: "chunk",
                data: {
                    label: c.title,
                    type: c.type,
                    connectionCount: 0,
                    tags: [] as string[]
                },
                position: pos,
                style: {
                    cursor: "pointer",
                    background: typeColor!.bg,
                    borderColor: typeColor!.border,
                    borderWidth: 1.5,
                    borderRadius: 10,
                    color: isDark ? "#e2e8f0" : "#1e293b",
                    fontSize: 12,
                    fontWeight: 500,
                    padding: "5px 10px",
                    whiteSpace: "nowrap" as const,
                    overflow: "hidden" as const,
                    textOverflow: "ellipsis" as const,
                    maxWidth: 250
                }
            };
        });

        const graphEdges: Edge[] = connections.map(conn => ({
            id: conn.id,
            source: conn.sourceId,
            target: conn.targetId,
            type: "floating",
            animated: true,
            data: { relation: conn.relation },
            style: {
                stroke: relationColor(conn.relation),
                strokeWidth: 1.5,
                opacity: 0.7
            }
        }));

        return { graphNodes, graphEdges };
    }, [savedGraph, graphData, isDark, TYPE_COLORS]);

    // Set nodes and edges when data is ready
    useEffect(() => {
        if (graphNodes.length > 0) {
            setNodes(graphNodes);
            setEdges(graphEdges);
        }
    }, [graphNodes, graphEdges, setNodes, setEdges]);

    // Fit view on first load
    useEffect(() => {
        if (graphNodes.length > 0 && !initialFitDoneRef.current) {
            setTimeout(() => fitView({ padding: 0.2 }), 100);
            initialFitDoneRef.current = true;
        }
    }, [graphNodes, fitView]);

    const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
        setSelectedChunkId(node.id);
    }, []);

    const handleSavePositions = useCallback(() => {
        const positions: Record<string, { x: number; y: number }> = {};
        for (const node of nodes) {
            positions[node.id] = node.position;
        }
        updatePositionsMutation.mutate(positions);
    }, [nodes, updatePositionsMutation]);

    const isLoading = isLoadingSavedGraph || isLoadingGraph;
    const sgName = (savedGraph as { name?: string } | null)?.name ?? "Saved Graph";
    const sgDescription = (savedGraph as { description?: string | null } | null)?.description;

    if (isLoading) {
        return (
            <div className="flex h-[calc(100vh-4rem)] items-center justify-center">
                <Spinner className="size-6" />
            </div>
        );
    }

    if (!savedGraph) {
        return (
            <div className="flex h-[calc(100vh-4rem)] flex-col items-center justify-center gap-4">
                <p className="text-muted-foreground">Saved graph not found</p>
                <Link to="/graph" className="text-primary text-sm hover:underline">
                    Back to graph
                </Link>
            </div>
        );
    }

    return (
        <div className="flex h-[calc(100vh-4rem)]">
            {/* Side panel for selected chunk */}
            {selectedChunkId && (
                <div className="shrink-0 overflow-hidden" style={{ width: 380 }}>
                    <GraphDetailPanel
                        chunkId={selectedChunkId}
                        onClose={() => setSelectedChunkId(null)}
                        onNavigateToChunk={(id: string) => setSelectedChunkId(id)}
                    />
                </div>
            )}

            {/* Main area */}
            <div className="relative flex-1">
                <ReactFlow
                    nodes={nodes}
                    edges={edges}
                    onNodesChange={onNodesChange}
                    onEdgesChange={onEdgesChange}
                    nodeTypes={NODE_TYPES}
                    edgeTypes={EDGE_TYPES}
                    onNodeClick={onNodeClick}
                    onPaneClick={() => setSelectedChunkId(null)}
                    fitView
                    minZoom={0.05}
                    colorMode={isDark ? "dark" : "light"}
                >
                    <Background
                        variant={BackgroundVariant.Dots}
                        gap={20}
                        size={1}
                        color={isDark ? "rgba(148,163,184,0.15)" : "rgba(100,116,139,0.2)"}
                    />
                    <Controls />
                    <MiniMap
                        nodeColor={node => {
                            if (node.id === selectedChunkId) return "#f472b6";
                            const style = node.style as Record<string, string> | undefined;
                            return style?.borderColor ?? "#475569";
                        }}
                        maskColor={isDark ? "rgba(0, 0, 0, 0.7)" : "rgba(255, 255, 255, 0.7)"}
                        pannable
                        zoomable
                    />
                </ReactFlow>

                {/* Header bar */}
                <div className="pointer-events-none absolute top-4 right-4 left-4 z-10 flex items-center justify-between">
                    <div className="pointer-events-auto flex items-center gap-3">
                        <Link
                            to="/graph"
                            className="bg-background/80 text-muted-foreground hover:text-foreground flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-xs backdrop-blur-sm"
                        >
                            <ArrowLeft className="size-3.5" />
                            Graph
                        </Link>
                        <div className="bg-background/80 rounded-md border px-3 py-1.5 backdrop-blur-sm">
                            <h1 className="text-sm font-semibold">{sgName}</h1>
                            {sgDescription && (
                                <p className="text-muted-foreground text-xs">{sgDescription}</p>
                            )}
                        </div>
                    </div>
                    <div className="pointer-events-auto flex items-center gap-2">
                        <button
                            onClick={handleSavePositions}
                            disabled={updatePositionsMutation.isPending}
                            className="bg-background/80 text-muted-foreground hover:text-foreground flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-xs backdrop-blur-sm"
                        >
                            <Edit2 className="size-3" />
                            {updatePositionsMutation.isPending ? "Saving..." : "Save positions"}
                        </button>
                        <button
                            onClick={() => deleteMutation.mutate()}
                            disabled={deleteMutation.isPending}
                            className="bg-background/80 text-destructive hover:bg-destructive/10 flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-xs backdrop-blur-sm"
                        >
                            <Trash2 className="size-3" />
                            Delete
                        </button>
                        <span className="text-muted-foreground bg-background/80 rounded-lg border px-3 py-1.5 text-xs backdrop-blur-sm">
                            {graphNodes.length} chunks
                        </span>
                    </div>
                </div>
            </div>
        </div>
    );
}

export function SavedGraphView() {
    return (
        <ReactFlowProvider>
            <SavedGraphViewInner />
        </ReactFlowProvider>
    );
}

export default SavedGraphView;
