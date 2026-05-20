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
import { GraphColumnView } from "@/features/graph/graph-column-view";
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

type ViewMode = "graph" | "columns";

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

    // --- View mode ---
    const [viewMode, setViewMode] = useState<ViewMode>("graph");

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
        <div className="flex h-[calc(100vh-4rem)] max-md:flex-col">
            {/* Detail panel — side on desktop, bottom sheet on mobile */}
            {gs.selectedChunkId && !isMobile && (
                <div
                    className="relative shrink-0 overflow-hidden transition-[width] duration-300 ease-in-out"
                    style={{ width: gs.panelWidth }}
                >
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
                </div>
            )}

            {/* Column view */}
            {viewMode === "columns" ? (
                <div className="relative flex-1 overflow-hidden">
                    <GraphColumnView
                        islandData={islandData}
                        data={data}
                        chunkHealthScores={chunkHealthScores}
                        chunkTags={chunkTags}
                        islandColors={islandColors}
                        selectedChunkId={gs.selectedChunkId}
                        onSelectChunk={id => dispatch({ type: "SET_SELECTED_CHUNK", id })}
                    />
                    {/* Top bar for column view */}
                    <div className="absolute inset-x-0 top-0 z-10 flex flex-wrap items-start justify-between gap-2 p-3">
                        <div className="bg-background/90 flex items-center gap-1 rounded-lg border px-3 py-1.5 text-xs backdrop-blur-sm">
                            <span className="text-foreground font-medium">Columns</span>
                        </div>
                        <div className="flex items-center gap-2">
                            {availableTagTypeIds.size > 1 && (
                                <select
                                    value={groupingTagTypeId ?? ""}
                                    onChange={e => setGroupingTagTypeId(e.target.value || null)}
                                    className="bg-background/90 rounded-md border px-2 py-1.5 text-xs backdrop-blur-sm"
                                >
                                    {[...availableTagTypeIds].map(id => (
                                        <option key={id} value={id}>{tagTypeNames.get(id) ?? id}</option>
                                    ))}
                                </select>
                            )}
                            <button
                                onClick={() => setViewMode("graph")}
                                className="bg-background/90 rounded-md border px-2 py-1.5 text-xs backdrop-blur-sm"
                            >
                                Graph
                            </button>
                        </div>
                    </div>
                </div>
            ) : (

            /* Main graph area */
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
                    {!isMobile && (
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
                    )}
                </ReactFlow>

                {/* Top bar: breadcrumbs + controls */}
                <div className="absolute inset-x-0 top-0 z-10 flex flex-wrap items-start justify-between gap-2 p-3">
                    {/* Breadcrumbs */}
                    <div className="bg-background/90 flex items-center gap-1 rounded-lg border px-3 py-1.5 text-xs backdrop-blur-sm">
                        {zoom.breadcrumbs.map((crumb, i) => (
                            <span key={i} className="flex items-center gap-1">
                                {i > 0 && <ChevronRight className="text-muted-foreground size-3" />}
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

                    {/* Right controls */}
                    <div className="flex flex-wrap items-center gap-2">
                        {zoom.level === "overview" && availableTagTypeIds.size > 1 && (
                            <>
                                <button
                                    onClick={() => dispatch({ type: "TOGGLE_HEATMAP" })}
                                    className={`rounded border px-2 py-1.5 text-xs ${
                                        gs.heatmapMode
                                            ? "border-amber-500 bg-amber-500/20 text-amber-300"
                                            : "bg-background/90 border backdrop-blur-sm"
                                    }`}
                                >
                                    Health
                                </button>
                                <select
                                    value={groupingTagTypeId ?? ""}
                                    onChange={e => setGroupingTagTypeId(e.target.value || null)}
                                    className="bg-background/90 rounded-md border px-2 py-1.5 text-xs backdrop-blur-sm"
                                >
                                    {[...availableTagTypeIds].map(id => (
                                        <option key={id} value={id}>
                                            {tagTypeNames.get(id) ?? id}
                                        </option>
                                    ))}
                                </select>
                            </>
                        )}
                        <div className="bg-background/90 flex items-center gap-1 rounded-md border backdrop-blur-sm">
                            <input
                                type="text"
                                placeholder="Search..."
                                value={gs.searchQuery}
                                onChange={e => dispatch({ type: "SET_SEARCH_QUERY", query: e.target.value })}
                                className="w-28 bg-transparent px-2 py-1.5 text-xs placeholder:text-muted-foreground sm:w-40"
                            />
                            {gs.searchQuery && (
                                <button
                                    onClick={() => dispatch({ type: "SET_SEARCH_QUERY", query: "" })}
                                    className="text-muted-foreground hover:text-foreground pr-2 text-xs"
                                >
                                    ×
                                </button>
                            )}
                        </div>
                        <button
                            onClick={() => setViewMode("columns")}
                            className="bg-background/90 rounded-md border px-2 py-1.5 text-xs backdrop-blur-sm"
                        >
                            Columns
                        </button>
                    </div>
                </div>

                {/* Bottom bar: legend + counts */}
                <div className="absolute inset-x-0 bottom-0 z-10 flex items-end justify-between p-3">
                    <div />
                    <div className="flex flex-col items-end gap-1.5">
                        <div className="text-muted-foreground hidden gap-3 text-[8px] sm:flex">
                            <span><span className="text-blue-400">━▸</span> depends_on</span>
                            <span><span className="text-green-400">━━</span> part_of</span>
                            <span><span className="text-purple-400">━━</span> extends</span>
                            <span><span className="text-red-400">╌╌</span> contradicts</span>
                        </div>
                        <span className="text-muted-foreground bg-background/80 rounded-lg border px-3 py-1.5 text-xs backdrop-blur-sm">
                            {layoutNodes.length} nodes · {layoutEdges.length} edges
                        </span>
                    </div>
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

            )}

            {/* Mobile bottom sheet detail panel */}
            {gs.selectedChunkId && isMobile && (
                <div className="border-t bg-background shrink-0 overflow-y-auto" style={{ maxHeight: "40vh" }}>
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
