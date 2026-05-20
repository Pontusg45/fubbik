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
    type Node,
} from "@xyflow/react";
import { ChevronRight } from "lucide-react";
import { useTheme } from "next-themes";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Spinner } from "@/components/ui/spinner";
import { GraphIslandNode } from "@/features/graph/graph-island-node";
import { GraphChunkCard } from "@/features/graph/graph-chunk-card";
import { GraphChunkDot } from "@/features/graph/graph-chunk-dot";
import { TypedEdge } from "@/features/graph/typed-edge";
import { GraphDetailPanel } from "@/features/graph/graph-detail-panel";
import { layoutIslands } from "@/features/graph/island-layout";
import { layoutNeighborhood, type NeighborhoodResult } from "@/features/graph/neighborhood-layout";
import { useGraphZoom } from "@/features/graph/use-graph-zoom";
import { useGraphData } from "@/features/graph/use-graph-data";
import { useGraphNodes } from "@/features/graph/use-graph-nodes";
import { useGraphState } from "@/features/graph/use-graph-state";

const NODE_TYPES = {
    island: GraphIslandNode,
    chunkCard: GraphChunkCard,
    chunkDot: GraphChunkDot,
};
const EDGE_TYPES = { typed: TypedEdge };

/** Map of island names to colors (deterministic palette). */
const ISLAND_PALETTE = [
    "#3b82f6", "#22c55e", "#f59e0b", "#ef4444", "#a855f7",
    "#06b6d4", "#ec4899", "#84cc16", "#f97316", "#6366f1",
];

function GraphViewInner() {
    const { resolvedTheme } = useTheme();
    const isDark = resolvedTheme !== "light";

    const { state: gs, dispatch } = useGraphState();
    const { fitView } = useReactFlow();

    const {
        data,
        isLoading,
        scopedChunkTags,
        availableTagTypeIds,
        groupingTagTypeId,
        setGroupingTagTypeId,
        islandData,
        initialFocusChunkId,
        initialIslandId,
        chunkHealthScores: computedChunkHealthScores,
        islandHealthScores: computedIslandHealthScores,
    } = useGraphData(dispatch);

    const {
        zoom,
        zoomToIsland,
        goBack,
        goToOverview,
        setFocusChunk,
    } = useGraphZoom(initialFocusChunkId ?? undefined, initialIslandId ?? undefined);

    // --- Island positions (memoized force simulation) ---
    const islandPositions = useMemo(() => {
        if (islandData.islands.length === 0) return {};
        return layoutIslands({
            islands: islandData.islands.map(i => ({ id: i.id, chunkCount: i.chunkIds.length })),
            bridges: islandData.bridges,
        });
    }, [islandData.islands, islandData.bridges]);

    // --- Neighborhood layout for zoomed-in view ---
    const neighborhoodResult = useMemo<NeighborhoodResult | null>(() => {
        if (zoom.level === "overview" || !zoom.focusChunkId || !data?.chunks) return null;
        return layoutNeighborhood({
            focusChunkId: zoom.focusChunkId,
            chunks: data.chunks,
            connections: data.connections ?? [],
        });
    }, [zoom.level, zoom.focusChunkId, data?.chunks, data?.connections]);

    // --- Island health scores and colors ---
    const islandHealthScores = computedIslandHealthScores;

    const islandColors = useMemo(() => {
        const m = new Map<string, string>();
        islandData.islands.forEach((island, i) => {
            m.set(island.id, ISLAND_PALETTE[i % ISLAND_PALETTE.length]!);
        });
        return m;
    }, [islandData.islands]);

    // --- Chunk metadata maps ---
    const chunkSummaries = useMemo(() => {
        const m = new Map<string, string | null>();
        for (const c of data?.chunks ?? []) {
            m.set(c.id, c.summary);
        }
        return m;
    }, [data?.chunks]);

    const chunkHealthScores = computedChunkHealthScores;

    const chunkTags = useMemo(() => {
        const m = new Map<string, Array<{ name: string; color: string }>>();
        for (const ct of scopedChunkTags as Array<{ chunkId: string; tagName: string; tagTypeColor?: string | null }>) {
            const existing = m.get(ct.chunkId) ?? [];
            existing.push({ name: ct.tagName, color: ct.tagTypeColor ?? "#6b7280" });
            m.set(ct.chunkId, existing);
        }
        return m;
    }, [scopedChunkTags]);

    // --- Build nodes/edges via the zoom-aware hook ---
    const { layoutNodes, layoutEdges } = useGraphNodes({
        zoomLevel: zoom.level,
        islands: islandData.islands,
        bridges: islandData.bridges,
        islandPositions,
        islandHealthScores,
        islandColors,
        neighborhood: neighborhoodResult,
        focusChunkId: zoom.focusChunkId,
        data,
        chunkSummaries,
        chunkHealthScores,
        chunkTags,
        filterTypes: gs.filterTypes,
        filterRelations: gs.filterRelations,
        heatmapMode: gs.heatmapMode,
    });

    // --- React Flow state ---
    const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
    const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
    const initialFitDoneRef = useRef(false);

    // Sync layout nodes/edges into React Flow state
    useEffect(() => {
        setNodes(layoutNodes);
        setEdges(layoutEdges);
    }, [layoutNodes, layoutEdges, setNodes, setEdges]);

    // Fit view on first paint
    const fitViewRef = useRef(fitView);
    fitViewRef.current = fitView;
    useEffect(() => {
        if (layoutNodes.length > 0 && !initialFitDoneRef.current) {
            initialFitDoneRef.current = true;
            const timer = setTimeout(() => fitViewRef.current({ padding: 0.15 }), 200);
            return () => clearTimeout(timer);
        }
    }, [layoutNodes.length]);

    // Re-fit when zoom level changes
    useEffect(() => {
        if (initialFitDoneRef.current) {
            const timer = setTimeout(() => fitViewRef.current({ padding: 0.15, duration: 400 }), 100);
            return () => clearTimeout(timer);
        }
    }, [zoom.level]);

    // --- Click handlers ---
    const onNodeClick = useCallback((_event: React.MouseEvent, node: Node) => {
        if (zoom.level === "overview" && node.id.startsWith("island-")) {
            // Click island -> zoom to neighborhood
            const islandId = node.id.replace("island-", "");
            const island = islandData.islands.find(i => i.id === islandId);
            if (island && island.chunkIds.length > 0) {
                zoomToIsland(islandId, island.chunkIds[0]!);
            }
            return;
        }

        // Click chunk -> select it for detail
        dispatch({ type: "SET_SELECTED_CHUNK", id: node.id });
        if (zoom.level === "neighborhood") {
            setFocusChunk(node.id);
        }
    }, [zoom.level, islandData.islands, zoomToIsland, dispatch, setFocusChunk]);

    const onPaneClick = useCallback(() => {
        dispatch({ type: "DESELECT_ALL" });
    }, [dispatch]);

    // --- Keyboard: Esc -> goBack, ? -> toggle help ---
    useEffect(() => {
        function handleKeyDown(e: KeyboardEvent) {
            if (e.key === "Escape") {
                if (gs.selectedChunkId) {
                    dispatch({ type: "SET_SELECTED_CHUNK", id: null });
                } else {
                    goBack();
                }
            }
            if (e.key === "?" && !e.ctrlKey && !e.metaKey) {
                dispatch({ type: "TOGGLE_HELP" });
            }
        }
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [gs.selectedChunkId, dispatch, goBack]);

    // --- Tag type names for the picker ---
    const tagTypeNames = useMemo(() => {
        const m = new Map<string, string>();
        for (const tt of data?.tagTypes ?? []) {
            m.set(tt.id, tt.name);
        }
        return m;
    }, [data?.tagTypes]);

    // --- Mobile detection ---
    const [isMobile, setIsMobile] = useState(false);
    useEffect(() => {
        const mql = window.matchMedia("(max-width: 768px)");
        setIsMobile(mql.matches);
        const handler = (e: MediaQueryListEvent) => setIsMobile(e.matches);
        mql.addEventListener("change", handler);
        return () => mql.removeEventListener("change", handler);
    }, []);

    if (isLoading) {
        return (
            <div className="flex h-[calc(100vh-4rem)] items-center justify-center">
                <div className="text-muted-foreground flex items-center gap-2">
                    <Spinner className="size-5" />
                    <span className="text-sm">Loading graph...</span>
                </div>
            </div>
        );
    }

    return (
        <div className="flex h-[calc(100vh-4rem)]">
            {/* Detail panel (desktop) */}
            {!isMobile && (
                <div
                    className={`relative shrink-0 overflow-hidden transition-[width] duration-300 ease-in-out ${gs.selectedChunkId ? "" : "w-0"}`}
                    style={gs.selectedChunkId ? { width: gs.panelWidth } : undefined}
                >
                    {gs.selectedChunkId && (
                        <div className="h-full" style={{ width: gs.panelWidth }}>
                            <GraphDetailPanel
                                chunkId={gs.selectedChunkId}
                                onClose={() => dispatch({ type: "SET_SELECTED_CHUNK", id: null })}
                                onNavigateToChunk={id => {
                                    dispatch({ type: "SET_SELECTED_CHUNK", id });
                                    setFocusChunk(id);
                                }}
                            />
                        </div>
                    )}
                    {gs.selectedChunkId && (
                        <div
                            className="hover:bg-primary/30 active:bg-primary/50 absolute top-0 right-0 h-full w-1 cursor-col-resize"
                            onMouseDown={e => {
                                e.preventDefault();
                                const startX = e.clientX;
                                const startWidth = gs.panelWidth;
                                function onMouseMove(ev: MouseEvent) {
                                    const newWidth = Math.max(280, Math.min(600, startWidth + ev.clientX - startX));
                                    dispatch({ type: "SET_PANEL_WIDTH", width: newWidth });
                                }
                                function onMouseUp() {
                                    document.removeEventListener("mousemove", onMouseMove);
                                    document.removeEventListener("mouseup", onMouseUp);
                                }
                                document.addEventListener("mousemove", onMouseMove);
                                document.addEventListener("mouseup", onMouseUp);
                            }}
                        />
                    )}
                </div>
            )}

            {/* Main graph area */}
            <div className="relative flex-1 touch-manipulation [&_.react-flow__handle]:invisible [&_.react-flow__node]:transition-[transform] [&_.react-flow__node]:duration-500 [&_.react-flow__node]:ease-out">
                <ReactFlow
                    nodes={nodes}
                    edges={edges}
                    nodeTypes={NODE_TYPES}
                    edgeTypes={EDGE_TYPES}
                    onNodesChange={onNodesChange}
                    onEdgesChange={onEdgesChange}
                    onNodeClick={onNodeClick}
                    onPaneClick={onPaneClick}
                    onlyRenderVisibleElements
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
                            if (node.id === gs.selectedChunkId) return "#f472b6";
                            const style = node.style as React.CSSProperties | undefined;
                            return style?.borderColor?.toString() ?? "#475569";
                        }}
                        maskColor={isDark ? "rgba(0, 0, 0, 0.7)" : "rgba(255, 255, 255, 0.7)"}
                        pannable
                        zoomable
                    />
                </ReactFlow>

                {/* Breadcrumb bar */}
                <div className="absolute top-4 left-4 z-10 flex items-center gap-1 rounded-lg border bg-background/90 px-3 py-1.5 text-xs backdrop-blur-sm">
                    {zoom.breadcrumbs.map((crumb, i) => (
                        <span key={i} className="flex items-center gap-1">
                            {i > 0 && <ChevronRight className="size-3 text-muted-foreground" />}
                            <button
                                className={`hover:text-foreground ${
                                    i === zoom.breadcrumbs.length - 1
                                        ? "text-foreground font-medium"
                                        : "text-muted-foreground"
                                }`}
                                onClick={() => {
                                    if (crumb.level === "overview") goToOverview();
                                    if (crumb.level === "neighborhood" && crumb.islandId && crumb.chunkId) {
                                        zoomToIsland(crumb.islandId, crumb.chunkId);
                                    }
                                }}
                            >
                                {crumb.label}
                            </button>
                        </span>
                    ))}
                </div>

                {/* Tag type picker + heatmap toggle */}
                {zoom.level === "overview" && availableTagTypeIds.size > 1 && (
                    <div className="absolute top-4 right-4 z-10 flex items-center gap-2">
                        <button
                            onClick={() => dispatch({ type: "TOGGLE_HEATMAP" })}
                            className={`rounded border px-2 py-1 text-xs ${
                                gs.heatmapMode
                                    ? "border-amber-500 bg-amber-500/20 text-amber-300"
                                    : "border-slate-600 bg-slate-800 text-slate-400"
                            }`}
                        >
                            Health
                        </button>
                        <select
                            value={groupingTagTypeId ?? ""}
                            onChange={e => setGroupingTagTypeId(e.target.value || null)}
                            className="rounded-md border bg-background/90 px-2 py-1.5 text-xs backdrop-blur-sm"
                        >
                            {[...availableTagTypeIds].map(id => (
                                <option key={id} value={id}>
                                    {tagTypeNames.get(id) ?? id}
                                </option>
                            ))}
                        </select>
                    </div>
                )}

                {/* Search input */}
                <div className="absolute right-3 top-12 z-20 flex items-center gap-2">
                    <input
                        type="text"
                        placeholder="Search chunks..."
                        value={gs.searchQuery}
                        onChange={e => dispatch({ type: "SET_SEARCH_QUERY", query: e.target.value })}
                        className="w-48 rounded border border-slate-600 bg-slate-800/80 px-2 py-1 text-xs text-slate-300 placeholder:text-slate-500"
                    />
                    {gs.searchQuery && (
                        <button onClick={() => dispatch({ type: "SET_SEARCH_QUERY", query: "" })} className="text-xs text-slate-500 hover:text-slate-300">
                            ×
                        </button>
                    )}
                </div>

                {/* Edge legend */}
                <div className="absolute bottom-12 right-3 z-20 flex gap-3 text-[8px] text-slate-500">
                    <span><span className="text-blue-400">━▸</span> depends_on</span>
                    <span><span className="text-green-400">━━</span> part_of</span>
                    <span><span className="text-purple-400">━━</span> extends</span>
                    <span><span className="text-red-400">╌╌</span> contradicts</span>
                </div>

                {/* Node/edge counts */}
                <div className="absolute bottom-4 right-4 z-10">
                    <span className="text-muted-foreground bg-background/80 rounded-lg border px-3 py-1.5 text-xs backdrop-blur-sm">
                        {layoutNodes.length} nodes / {layoutEdges.length} edges
                    </span>
                </div>

                {/* Help overlay */}
                {gs.showHelp && (
                    <div
                        className="bg-background/80 absolute inset-0 z-30 flex items-center justify-center backdrop-blur-sm"
                        onClick={() => dispatch({ type: "TOGGLE_HELP" })}
                    >
                        <div className="bg-background max-w-sm rounded-lg border p-6 shadow-lg" onClick={e => e.stopPropagation()}>
                            <h3 className="mb-4 text-sm font-semibold">Keyboard Shortcuts</h3>
                            <div className="space-y-2 text-xs">
                                {[
                                    ["Click island", "Zoom into neighborhood"],
                                    ["Click chunk", "Select & show details"],
                                    ["Escape", "Go back / deselect"],
                                    ["?", "Toggle this help"],
                                ].map(([key, desc]) => (
                                    <div key={key} className="flex items-center justify-between gap-4">
                                        <kbd className="bg-muted rounded border px-1.5 py-0.5 font-mono text-[10px]">{key}</kbd>
                                        <span className="text-muted-foreground text-right">{desc}</span>
                                    </div>
                                ))}
                            </div>
                            <button
                                onClick={() => dispatch({ type: "TOGGLE_HELP" })}
                                className="bg-primary text-primary-foreground mt-4 w-full rounded-md px-3 py-1.5 text-xs"
                            >
                                Close
                            </button>
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}

export function GraphView() {
    return (
        <ReactFlowProvider>
            <GraphViewInner />
        </ReactFlowProvider>
    );
}

export default GraphView;
