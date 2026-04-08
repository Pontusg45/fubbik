import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
    Background,
    BackgroundVariant,
    ReactFlow,
    ReactFlowProvider,
    type Edge,
    type Node,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Handle, Position } from "@xyflow/react";
import { useTheme } from "next-themes";
import { useMemo } from "react";

import { relationColor } from "@/features/chunks/relation-colors";
import { runForceLayout } from "@/features/graph/force-layout";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface SearchGraphProps {
    chunkIds: string[];
    chunks: Array<{ id: string; title: string; type: string }>;
}

const TYPE_COLORS_DARK: Record<string, { bg: string; border: string; text: string }> = {
    note: { bg: "#1e293b", border: "#475569", text: "#94a3b8" },
    guide: { bg: "#1e1b4b", border: "#6366f1", text: "#a5b4fc" },
    reference: { bg: "#042f2e", border: "#14b8a6", text: "#5eead4" },
    document: { bg: "#172554", border: "#3b82f6", text: "#93c5fd" },
    schema: { bg: "#1c1917", border: "#f59e0b", text: "#fcd34d" },
    checklist: { bg: "#1a2e05", border: "#84cc16", text: "#bef264" },
};

const TYPE_COLORS_LIGHT: Record<string, { bg: string; border: string; text: string }> = {
    note: { bg: "#f1f5f9", border: "#94a3b8", text: "#475569" },
    guide: { bg: "#eef2ff", border: "#6366f1", text: "#4338ca" },
    reference: { bg: "#f0fdfa", border: "#14b8a6", text: "#0f766e" },
    document: { bg: "#eff6ff", border: "#3b82f6", text: "#1d4ed8" },
    schema: { bg: "#fefce8", border: "#f59e0b", text: "#b45309" },
    checklist: { bg: "#f7fee7", border: "#84cc16", text: "#4d7c0f" },
};

const DEFAULT_COLOR_DARK = { bg: "#1e293b", border: "#475569", text: "#94a3b8" };
const DEFAULT_COLOR_LIGHT = { bg: "#f8fafc", border: "#cbd5e1", text: "#64748b" };

/** Mini node for the search graph — compact, title-only */
function SearchMiniNode({ data }: { data: Record<string, unknown> }) {
    const nodeData = data as { label: string; colors: { bg: string; border: string; text: string } };
    return (
        <>
            <Handle type="target" position={Position.Top} style={{ opacity: 0 }} />
            <Handle type="target" position={Position.Left} id="left-target" style={{ opacity: 0 }} />
            <Handle type="source" position={Position.Right} id="right-source" style={{ opacity: 0 }} />
            <Handle type="source" position={Position.Bottom} style={{ opacity: 0 }} />
            <div
                className="max-w-[120px] truncate rounded px-2 py-1 text-[10px] font-medium leading-tight"
                style={{
                    background: nodeData.colors.bg,
                    border: `1px solid ${nodeData.colors.border}`,
                    color: nodeData.colors.text,
                }}
                title={nodeData.label}
            >
                {nodeData.label}
            </div>
        </>
    );
}

const MINI_NODE_TYPES = { miniChunk: SearchMiniNode };

function SearchGraphInner({ chunkIds, chunks }: SearchGraphProps) {
    const { resolvedTheme } = useTheme();
    const isDark = resolvedTheme !== "light";
    const TYPE_COLORS = isDark ? TYPE_COLORS_DARK : TYPE_COLORS_LIGHT;
    const DEFAULT_COLOR = isDark ? DEFAULT_COLOR_DARK : DEFAULT_COLOR_LIGHT;
    const navigate = useNavigate();

    const { data: graphData, isLoading } = useQuery({
        queryKey: ["graph", "search-minimap"],
        queryFn: async () => {
            return unwrapEden(await api.api.graph.get({ query: {} }));
        },
        staleTime: 30_000,
    });

    const chunkIdSet = useMemo(() => new Set(chunkIds), [chunkIds]);
    const chunkMap = useMemo(
        () => new Map(chunks.map(c => [c.id, c])),
        [chunks]
    );

    const { nodes, edges } = useMemo(() => {
        if (!graphData) return { nodes: [], edges: [] };

        // Filter connections to only those between result chunks
        const relevantEdges = (graphData.connections ?? []).filter(
            c => chunkIdSet.has(c.sourceId) && chunkIdSet.has(c.targetId)
        );

        // Build simple node list for layout
        const layoutNodes = chunkIds.map(id => ({ id, type: chunkMap.get(id)?.type ?? "note" }));
        const layoutEdges = relevantEdges.map(c => ({
            source: c.sourceId,
            target: c.targetId,
            relation: c.relation,
        }));

        // Run force layout
        const positions = runForceLayout(layoutNodes, layoutEdges);

        const rfNodes: Node[] = chunkIds.map(id => {
            const chunk = chunkMap.get(id);
            const colors = TYPE_COLORS[chunk?.type ?? ""] ?? DEFAULT_COLOR;
            const pos = positions[id] ?? { x: 0, y: 0 };
            return {
                id,
                type: "miniChunk",
                position: pos,
                data: {
                    label: chunk?.title ?? id,
                    colors,
                },
            };
        });

        const rfEdges: Edge[] = relevantEdges.map(c => ({
            id: `${c.sourceId}-${c.targetId}-${c.relation}`,
            source: c.sourceId,
            target: c.targetId,
            style: {
                stroke: relationColor(c.relation),
                strokeWidth: 1.5,
                strokeOpacity: 0.7,
            },
        }));

        return { nodes: rfNodes, edges: rfEdges };
    }, [graphData, chunkIds, chunkIdSet, chunkMap, TYPE_COLORS, DEFAULT_COLOR]);

    if (isLoading) {
        return (
            <div
                className="flex items-center justify-center rounded-lg border bg-muted/30"
                style={{ height: 220 }}
            >
                <span className="text-xs text-muted-foreground">Loading graph...</span>
            </div>
        );
    }

    if (nodes.length === 0) {
        return (
            <div
                className="flex items-center justify-center rounded-lg border bg-muted/30"
                style={{ height: 220 }}
            >
                <span className="text-xs text-muted-foreground">No graph data for these results</span>
            </div>
        );
    }

    const connectedCount = edges.length;

    return (
        <div className="overflow-hidden rounded-lg border" style={{ height: 220 }}>
            <ReactFlow
                nodes={nodes}
                edges={edges}
                nodeTypes={MINI_NODE_TYPES}
                fitView
                fitViewOptions={{ padding: 0.2 }}
                nodesDraggable={false}
                nodesConnectable={false}
                elementsSelectable={false}
                panOnScroll={false}
                zoomOnScroll={false}
                zoomOnPinch={false}
                preventScrolling={false}
                onNodeClick={(_, node) => {
                    void navigate({ to: "/chunks/$chunkId", params: { chunkId: node.id } });
                }}
                proOptions={{ hideAttribution: true }}
                colorMode={isDark ? "dark" : "light"}
            >
                <Background
                    variant={BackgroundVariant.Dots}
                    gap={16}
                    size={1}
                    color={isDark ? "#334155" : "#e2e8f0"}
                />
                <div className="absolute bottom-2 left-2 z-10 rounded bg-background/80 px-2 py-1 text-[10px] text-muted-foreground backdrop-blur-sm">
                    {nodes.length} chunks · {connectedCount} connection{connectedCount !== 1 ? "s" : ""}
                </div>
            </ReactFlow>
        </div>
    );
}

export function SearchGraph(props: SearchGraphProps) {
    return (
        <ReactFlowProvider>
            <SearchGraphInner {...props} />
        </ReactFlowProvider>
    );
}
