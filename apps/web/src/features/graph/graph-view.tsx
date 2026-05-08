import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import "@xyflow/react/dist/style.css";
import {
    Background,
    BackgroundVariant,
    ConnectionMode,
    Controls,
    MiniMap,
    ReactFlow,
    ReactFlowProvider,
    useEdgesState,
    useNodesState,
    useReactFlow,
    type Connection,
    type Edge,
    type Node,
    type Viewport
} from "@xyflow/react";
import { toPng } from "html-to-image";
import { Route } from "lucide-react";
import { useTheme } from "next-themes";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Popover, PopoverTrigger, PopoverPopup } from "@/components/ui/popover";
import { Sheet, SheetContent } from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { FloatingEdge } from "@/features/graph/floating-edge";
import { GraphDetailPanel } from "@/features/graph/graph-detail-panel";
import { GraphFilters } from "@/features/graph/graph-filters";
import { GraphSettingsPanel } from "@/features/graph/graph-settings-panel";
import { GraphFilterDialog, type GraphFilterValues } from "@/features/graph/graph-filter-dialog";
import { isGroupNodeId } from "@/features/graph/group-strategies";
import { GraphClusterNode } from "@/features/graph/graph-cluster-node";
import { mark, measure } from "@/features/graph/graph-timings";
import { MermaidExportModal } from "@/features/graph/mermaid-export-modal";
import { GraphLegend } from "@/features/graph/graph-legend";
import { GraphMetrics } from "@/features/graph/graph-metrics";
import { GraphNode } from "@/features/graph/graph-node";
import { GraphGroupNode } from "@/features/graph/graph-group-node";
import { GraphContextMenu } from "@/features/graph/graph-context-menu";
import { GraphWelcome } from "@/features/graph/graph-welcome";
import type { LayoutAlgorithm } from "@/features/graph/layouts";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { ChangeConnectionDialog, SaveViewDialog, SaveCustomGraphDialog } from "./graph-dialogs";
import { GraphTimeline } from "./graph-timeline";
import { PathPanel } from "./path-panel";
import { useGraphKeyboard } from "./use-graph-keyboard";
import { useGraphState } from "./use-graph-state";
import { useSavedGraphViews } from "./use-saved-views";
import { useGraphData } from "./use-graph-data";
import { useGraphGrouping } from "./use-graph-grouping";
import { useGraphLayout } from "./use-graph-layout";
import { useGraphNodes } from "./use-graph-nodes";
import { useGraphInteractions } from "./use-graph-interactions";
import { useGraphStyling } from "./use-graph-styling";


const EDGE_TYPES = { floating: FloatingEdge };
const NODE_TYPES = { chunk: GraphNode, group: GraphGroupNode, cluster: GraphClusterNode };

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

function GraphViewInner() {
    const { resolvedTheme } = useTheme();
    const isDark = resolvedTheme !== "light";
    const TYPE_COLORS = isDark ? TYPE_COLORS_DARK : TYPE_COLORS_LIGHT;

    const zoomRef = useRef(1);
    const { state: gs, dispatch, handleTimelineCutoff } = useGraphState();
    const { setCenter, getZoom, fitView } = useReactFlow();
    const initialFitDoneRef = useRef(false);
    const navigate = useNavigate();
    const queryClient = useQueryClient();

    // Destructure frequently-used state for readability
    const {
        selectedChunkId, multiSelectedIds, pendingConnection,
        exploreMode, exploredNodeIds, pathStartId, pathEndId,
        showHelp, showWelcome, showDeleteConfirm, focusedNodeId,
        collapsedParents, showPathPanel, showSaveDialog, viewName,
        filterTypes, filterRelations, searchQuery, activeTagTypeIds, showUngrouped,
        layoutAlgorithm, bundleEdges, useMainThread, timelineCutoff, panelWidth, edgeAnimated,
        expandedClusters,
    } = gs;

    const createConnectionMutation = useMutation({
        mutationFn: async ({ sourceId, targetId, relation }: { sourceId: string; targetId: string; relation: string }) => {
            const { error } = await api.api.connections.post({ sourceId, targetId, relation });
            if (error) throw new Error("Failed to create connection");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["graph"] });
            dispatch({ type: "SET_PENDING_CONNECTION", connection: null });
        }
    });

    const deleteManyMutation = useMutation({
        mutationFn: async (ids: string[]) => {
            return unwrapEden(await api.api.chunks.bulk.delete({ ids }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["graph"] });
            dispatch({ type: "CLEAR_MULTI_SELECT" });
        }
    });

    const onConnect = useCallback((connection: Connection) => {
        if (!connection.source || !connection.target) return;
        dispatch({ type: "SET_PENDING_CONNECTION", connection: { source: connection.source, target: connection.target } });
    }, [dispatch]);

    // Saved custom graphs (server-side)
    const savedGraphsQuery = useQuery({
        queryKey: ["saved-graphs"],
        queryFn: async () => unwrapEden(await api.api["saved-graphs"].get({}))
    });

    const saveCustomGraphMutation = useMutation({
        mutationFn: async (body: {
            name: string;
            chunkIds: string[];
            positions: Record<string, { x: number; y: number }>;
            layoutAlgorithm: string;
            codebaseId?: string | null;
        }) => {
            return unwrapEden(await api.api["saved-graphs"].post(body));
        },
        onSuccess: (data) => {
            queryClient.invalidateQueries({ queryKey: ["saved-graphs"] });
            toast.success("Custom graph saved", {
                action: {
                    label: "Open",
                    onClick: () => navigate({ to: "/graph/$graphId", params: { graphId: (data as { id: string }).id } })
                }
            });
        }
    });

    const deleteCustomGraphMutation = useMutation({
        mutationFn: async (id: string) => {
            return unwrapEden(await api.api["saved-graphs"]({ id }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["saved-graphs"] });
        }
    });

    const [showSaveCustomDialog, setShowSaveCustomDialog] = useState(false);
    const [customGraphName, setCustomGraphName] = useState("");

    // Focus mode: double-click a node to dim everything beyond 2 hops
    const [focusModeNodeId, setFocusModeNodeId] = useState<string | null>(null);

    const [isMobile, setIsMobile] = useState(false);

    useEffect(() => {
        const mql = window.matchMedia("(max-width: 768px)");
        setIsMobile(mql.matches);
        function handler(e: MediaQueryListEvent) {
            setIsMobile(e.matches);
        }
        mql.addEventListener("change", handler);
        return () => mql.removeEventListener("change", handler);
    }, []);

    // --- Composable hooks pipeline: data -> grouping -> layout -> nodes -> interactions -> styling ---

    const {
        data,
        isLoading,
        codebaseId,
        prefilter,
        filterDialogOpen,
        setFilterDialogOpen,
        scopedChunkTags,
        availableTagTypeIds,
    } = useGraphData(dispatch);

    const debouncedSearchQuery = useDebouncedValue(searchQuery, 150);

    const {
        groupingMode,
        groupResult,
        tagGroups,
        clusters,
        shouldCluster,
        clusterVisibleChunkIds,
        chunkTagGroupMap,
    } = useGraphGrouping({
        data,
        prefilter,
        activeTagTypeIds,
        availableTagTypeIds,
        expandedClusters,
        scopedChunkTags,
        TYPE_COLORS,
    });

    const {
        filteredGraph,
        layoutPositions,
        isLayouting,
    } = useGraphLayout({
        data,
        scopedChunkTags,
        prefilter,
        filterTypes,
        filterRelations,
        collapsedParents,
        exploreMode,
        exploredNodeIds,
        timelineCutoff,
        layoutAlgorithm,
        useMainThread,
        selectedChunkId,
        tagGroups,
        groupingMode,
    });

    // Dragged node positions (persist across layout changes)
    const [draggedPositions, setDraggedPositions] = useState<Map<string, { x: number; y: number }>>(new Map());
    const groupDragStartRef = useRef<{
        groupId: string;
        startPos: { x: number; y: number };
        childStart: Map<string, { x: number; y: number }>;
    } | null>(null);

    const {
        layoutNodes,
        layoutEdges,
        groupToChunkIds,
        legendTypeCounts,
        legendRelationCounts,
        measuredNodeSizesRef,
        setMeasuredSizesVersion,
    } = useGraphNodes({
        filteredGraph,
        layoutPositions,
        draggedPositions,
        isDark,
        TYPE_COLORS,
        collapsedParents,
        bundleEdges,
        tagGroups,
        groupResult,
        data,
        edgeAnimated,
        showUngrouped,
        shouldCluster,
        clusters,
        clusterVisibleChunkIds,
        dispatch,
    });

    const {
        searchMatchIds,
        focusNeighbors,
        focusModeNeighbors,
        selectedEdgeIds,
        selectedNeighborNodes,
        pathResult,
    } = useGraphInteractions({
        layoutNodes,
        layoutEdges,
        debouncedSearchQuery,
        focusedNodeId,
        focusModeNodeId,
        selectedChunkId,
        pathStartId,
        pathEndId,
    });

    // Auto-center on first search match
    useEffect(() => {
        if (searchMatchIds.size === 0) return;
        const firstMatchId = [...searchMatchIds][0];
        const matchNode = layoutNodes.find(n => n.id === firstMatchId);
        if (matchNode?.position) {
            setCenter(matchNode.position.x, matchNode.position.y, { zoom: getZoom(), duration: 400 });
        }
    }, [searchMatchIds, layoutNodes, setCenter, getZoom]);

    const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
    const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
    const [mermaidModalOpen, setMermaidModalOpen] = useState(false);

    // Keep measured sizes ref in sync for group bounding box calculation
    useEffect(() => {
        const sizes = new Map<string, { w: number; h: number }>();
        for (const n of nodes) {
            if (n.measured?.width && n.measured?.height) {
                sizes.set(n.id, { w: n.measured.width, h: n.measured.height });
            }
        }
        if (sizes.size > 0 && sizes.size !== measuredNodeSizesRef.current.size) {
            measuredNodeSizesRef.current = sizes;
            setMeasuredSizesVersion(v => v + 1);
        }
    }, [nodes, measuredNodeSizesRef, setMeasuredSizesVersion]);

    /** Preserve React DOM identity so CSS transform transitions work. */
    function mergeNodes(newNodes: Node[]) {
        setNodes(prev => {
            if (prev.length === 0) return newNodes;
            const prevMap = new Map(prev.map(n => [n.id, n]));
            return newNodes.map(node => {
                const existing = prevMap.get(node.id);
                if (existing) {
                    return { ...existing, position: node.position, style: node.style, data: node.data, type: node.type };
                }
                return node;
            });
        });
    }

    useGraphStyling({
        layoutNodes,
        layoutEdges,
        debouncedSearchQuery,
        focusModeNeighbors,
        focusNeighbors,
        selectedNeighborNodes,
        selectedEdgeIds,
        multiSelectedIds,
        pathResult,
        chunkTagGroupMap,
        selectedChunkId,
        mergeNodes,
        setEdges,
    });

    const [hoveredNode, setHoveredNode] = useState<{ id: string; x: number; y: number } | null>(null);
    const [contextMenu, setContextMenu] = useState<{ x: number; y: number; nodeId?: string } | null>(null);

    const onMoveEnd = useCallback((_: unknown, viewport: Viewport) => {
        zoomRef.current = viewport.zoom;
    }, []);

    const onInit = useCallback(() => {
        // no-op: let fitView run after layout positions arrive
    }, []);

    const chunkMap = useMemo(() => {
        const map = new Map<string, { title: string; type: string; tags: string[]; summary: string | null }>();
        for (const c of data?.chunks ?? []) {
            map.set(c.id, { title: c.title, type: c.type, tags: [], summary: c.summary });
        }
        return map;
    }, [data]);

    // Fit view once after first layout positions arrive
    const fitViewRef = useRef(fitView);
    fitViewRef.current = fitView;
    useEffect(() => {
        if (layoutPositions && !initialFitDoneRef.current) {
            initialFitDoneRef.current = true;
            mark("flow-painted");
            measure("time-to-first-node", "mount", "flow-painted");
            const timer = setTimeout(() => fitViewRef.current({ padding: 0.1 }), 200);
            return () => clearTimeout(timer);
        }
    }, [layoutPositions]);

    // eslint-disable-next-line react-hooks/exhaustive-deps
    useEffect(() => {
        if (!selectedChunkId) return;
        const node = nodes.find(n => n.id === selectedChunkId);
        if (!node) return;
        const x = node.position.x + (node.measured?.width ?? 180) / 2;
        const y = node.position.y + (node.measured?.height ?? 40) / 2;
        setCenter(x, y, { duration: 400, zoom: getZoom() });
    }, [selectedChunkId, nodes, getZoom, setCenter]);

    useGraphKeyboard({
        selectedChunkId,
        focusedNodeId,
        focusModeNodeId,
        onExitFocusMode: () => setFocusModeNodeId(null),
        pathStartId,
        pathEndId,
        multiSelectedIds,
        layoutEdges,
        dispatch,
    });

    // Toggle helpers
    function toggleRelation(r: string) {
        dispatch({ type: "TOGGLE_FILTER_RELATION", relation: r });
    }
    function toggleTypePrefilterFromLegend(typeId: string) {
        const next = prefilter.types.includes(typeId)
            ? prefilter.types.filter(t => t !== typeId)
            : [...prefilter.types, typeId];
        handleFilterApply({ ...prefilter, types: next });
    }

    const dismissWelcome = () => {
        dispatch({ type: "SET_SHOW_WELCOME", show: false });
    };

    // Saved views
    const { views: savedViews, saveView, deleteView: deleteSavedView } = useSavedGraphViews();

    function handleExportImage() {
        const viewport = document.querySelector(".react-flow__viewport") as HTMLElement | null;
        if (!viewport) return;
        toPng(viewport, {
            backgroundColor: isDark ? "#0f172a" : "#ffffff",
            quality: 1,
            pixelRatio: 2
        }).then(dataUrl => {
            const link = document.createElement("a");
            link.download = "graph.png";
            link.href = dataUrl;
            link.click();
        });
    }

    function handleFilterApply(values: GraphFilterValues) {
        navigate({
            to: "/graph",
            search: (prev: Record<string, unknown>) => ({
                ...prev,
                tags: values.tags.length > 0 ? values.tags.join(",") : undefined,
                types: values.types.length > 0 ? values.types.join(",") : undefined,
                focus: values.focusChunkId ?? undefined,
                depth: values.focusChunkId ? values.depth : undefined,
                groupBy: values.groupBy,
                tagTypeId: values.tagTypeId ?? undefined,
                all: undefined
            })
        });
    }
    function handleShowEverything() {
        navigate({
            to: "/graph",
            search: (prev: Record<string, unknown>) => ({
                ...prev,
                tags: undefined,
                types: undefined,
                focus: undefined,
                depth: undefined,
                groupBy: undefined,
                tagTypeId: undefined,
                all: 1
            })
        });
    }

    if (isLoading) {
        return (
            <div className="flex h-[calc(100vh-4rem)] items-center justify-center">
                <p className="text-muted-foreground">Loading graph...</p>
            </div>
        );
    }

    const showLayoutSpinner = isLayouting && !layoutPositions;

    return (
        <div className="flex h-[calc(100vh-4rem)]">
            {!isMobile && (
                <div
                    className={`relative shrink-0 overflow-hidden transition-[width] duration-300 ease-in-out ${selectedChunkId ? "" : "w-0"}`}
                    style={selectedChunkId ? { width: panelWidth } : undefined}
                >
                    {selectedChunkId && (
                        <div className="h-full" style={{ width: panelWidth }}>
                            <GraphDetailPanel
                                chunkId={selectedChunkId}
                                onClose={() => dispatch({ type: "SET_SELECTED_CHUNK", id: null })}
                                onNavigateToChunk={id => dispatch({ type: "SELECT_AND_FOCUS_NODE", id })}
                            />
                        </div>
                    )}
                    {selectedChunkId && (
                        <div
                            className="hover:bg-primary/30 active:bg-primary/50 absolute top-0 right-0 h-full w-1 cursor-col-resize"
                            onMouseDown={e => {
                                e.preventDefault();
                                const startX = e.clientX;
                                const startWidth = panelWidth;
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
            {isMobile && (
                <Sheet
                    open={!!selectedChunkId}
                    onOpenChange={open => {
                        if (!open) dispatch({ type: "SET_SELECTED_CHUNK", id: null });
                    }}
                >
                    <SheetContent side="bottom" showCloseButton={false} className="h-[70vh] overflow-y-auto p-0">
                        {selectedChunkId && (
                            <GraphDetailPanel
                                chunkId={selectedChunkId}
                                onClose={() => dispatch({ type: "SET_SELECTED_CHUNK", id: null })}
                                onNavigateToChunk={id => dispatch({ type: "SELECT_AND_FOCUS_NODE", id })}
                            />
                        )}
                    </SheetContent>
                </Sheet>
            )}
            <div className="relative flex-1 touch-manipulation [&_.react-flow__handle]:invisible [&_.react-flow__handle]:transition-all [&_.react-flow__handle]:duration-150 [&_.react-flow__node]:transition-[transform] [&_.react-flow__node]:duration-500 [&_.react-flow__node]:ease-out [&_.react-flow__node:hover_.react-flow__handle]:!visible">
                {showLayoutSpinner && (
                    <div className="bg-background/60 absolute inset-0 z-50 flex items-center justify-center backdrop-blur-sm">
                        <div className="text-muted-foreground flex items-center gap-2">
                            <Spinner className="size-5" />
                            <span className="text-sm">Computing layout...</span>
                        </div>
                    </div>
                )}
                    <ReactFlow
                        nodes={nodes}
                        edges={edges}
                        nodeTypes={NODE_TYPES}
                        edgeTypes={EDGE_TYPES}
                        onNodesChange={onNodesChange}
                        onEdgesChange={onEdgesChange}
                        onConnect={onConnect}
                        connectionMode={ConnectionMode.Loose}
                        onlyRenderVisibleElements
                        onNodeClick={(event, node) => {
                            if (isGroupNodeId(node.id)) return;
                            if (node.type === "cluster") return;
                            if (event.shiftKey) {
                                dispatch({ type: "TOGGLE_MULTI_SELECT", id: node.id });
                                return;
                            }
                            if (multiSelectedIds.size > 0) {
                                dispatch({ type: "CLEAR_MULTI_SELECT" });
                            }
                            if (event.altKey) {
                                if (!pathStartId) {
                                    dispatch({ type: "SET_PATH_START", id: node.id });
                                    dispatch({ type: "SET_PATH_END", id: null });
                                } else if (!pathEndId) {
                                    dispatch({ type: "SET_PATH_END", id: node.id });
                                } else {
                                    dispatch({ type: "SET_PATH_START", id: node.id });
                                    dispatch({ type: "SET_PATH_END", id: null });
                                }
                                return;
                            }
                            if (exploreMode) {
                                const neighborIds = [node.id];
                                for (const edge of layoutEdges) {
                                    if (edge.source === node.id) neighborIds.push(edge.target);
                                    if (edge.target === node.id) neighborIds.push(edge.source);
                                }
                                dispatch({ type: "ADD_EXPLORED_NODES", ids: neighborIds });
                                dispatch({ type: "SELECT_AND_FOCUS_NODE", id: node.id });
                                return;
                            }
                            dispatch({ type: "CLEAR_PATH" });
                            dispatch({ type: "SELECT_AND_FOCUS_NODE", id: node.id });
                        }}
                        onPaneClick={() => {
                            dispatch({ type: "DESELECT_ALL" });
                            setFocusModeNodeId(null);
                            setContextMenu(null);
                        }}
                        onNodeDoubleClick={(_, node) => {
                            if (isGroupNodeId(node.id)) return;
                            if (node.type === "cluster") return;
                            if (focusModeNodeId === node.id) {
                                setFocusModeNodeId(null);
                            } else {
                                setFocusModeNodeId(node.id);
                            }
                        }}
                        onNodeMouseEnter={(event, node) => {
                            setHoveredNode({ id: node.id, x: event.clientX, y: event.clientY });
                        }}
                        onNodeDragStart={(_, node) => {
                            if (isGroupNodeId(node.id)) {
                                const childIds = groupToChunkIds.get(node.id) ?? [];
                                const childStart = new Map<string, { x: number; y: number }>();
                                for (const cid of childIds) {
                                    const childNode = nodes.find(n => n.id === cid);
                                    if (childNode) childStart.set(cid, { ...childNode.position });
                                }
                                groupDragStartRef.current = {
                                    groupId: node.id,
                                    startPos: { ...node.position },
                                    childStart
                                };
                            }
                        }}
                        onNodeDrag={(_, node) => {
                            if (isGroupNodeId(node.id) && groupDragStartRef.current?.groupId === node.id) {
                                const dx = node.position.x - groupDragStartRef.current.startPos.x;
                                const dy = node.position.y - groupDragStartRef.current.startPos.y;
                                const childStart = groupDragStartRef.current.childStart;
                                if (childStart.size === 0) return;
                                setNodes(prev => prev.map(n => {
                                    const start = childStart.get(n.id);
                                    if (!start) return n;
                                    return { ...n, position: { x: start.x + dx, y: start.y + dy } };
                                }));
                            }
                        }}
                        onNodeDragStop={(_, node) => {
                            if (isGroupNodeId(node.id) && groupDragStartRef.current?.groupId === node.id) {
                                groupDragStartRef.current = null;
                                const childIds = groupToChunkIds.get(node.id);
                                if (!childIds) return;
                                setDraggedPositions(prev => {
                                    const next = new Map(prev);
                                    next.set(node.id, node.position);
                                    for (const cid of childIds) {
                                        const childNode = nodes.find(n => n.id === cid);
                                        if (childNode) {
                                            next.set(cid, childNode.position);
                                        }
                                    }
                                    return next;
                                });
                                return;
                            }
                            setDraggedPositions(prev => {
                                const next = new Map(prev);
                                next.set(node.id, node.position);
                                return next;
                            });
                        }}
                        onNodeMouseLeave={() => setHoveredNode(null)}
                        onNodeContextMenu={(event, node) => {
                            event.preventDefault();
                            if (isGroupNodeId(node.id)) return;
                            if (node.type === "cluster") return;
                            setContextMenu({ x: event.clientX, y: event.clientY, nodeId: node.id });
                        }}
                        onPaneContextMenu={(event) => {
                            event.preventDefault();
                            setContextMenu({ x: event.clientX, y: event.clientY });
                        }}
                        onMoveEnd={onMoveEnd}
                        onEdgeMouseEnter={(_, edge) => {
                            setEdges(es =>
                                es.map(e =>
                                    e.id === edge.id
                                        ? {
                                              ...e,
                                              label: (e.data as { relation?: string })?.relation,
                                              labelStyle: {
                                                  fill: (e.style as Record<string, string>)?.stroke,
                                                  fontSize: 10,
                                                  fontWeight: 500
                                              },
                                              labelBgStyle: { fill: isDark ? "rgba(0,0,0,0.6)" : "rgba(255,255,255,0.8)", fillOpacity: 0.8 }
                                          }
                                        : e
                                )
                            );
                        }}
                        onEdgeMouseLeave={(_, edge) => {
                            setEdges(es =>
                                es.map(e =>
                                    e.id === edge.id ? { ...e, label: undefined, labelStyle: undefined, labelBgStyle: undefined } : e
                                )
                            );
                        }}
                        onInit={onInit}
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


                {/* Context menu */}
                {contextMenu && (
                    <GraphContextMenu
                        x={contextMenu.x}
                        y={contextMenu.y}
                        nodeId={contextMenu.nodeId}
                        onClose={() => setContextMenu(null)}
                        onFitView={() => fitView()}
                        onResetLayout={() => {
                            setDraggedPositions(new Map());
                        }}
                        onDelete={(nodeId) => {
                            dispatch({ type: "TOGGLE_MULTI_SELECT", id: nodeId });
                            dispatch({ type: "SET_SHOW_DELETE_CONFIRM", show: true });
                        }}
                    />
                )}

                {/* Welcome overlay for first-time visitors */}
                {showWelcome && <GraphWelcome onDismiss={dismissWelcome} />}

                {/* Keyboard shortcut help overlay */}
                {showHelp && (
                    <div
                        className="bg-background/80 absolute inset-0 z-30 flex items-center justify-center backdrop-blur-sm"
                        onClick={() => dispatch({ type: "TOGGLE_HELP" })}
                    >
                        <div className="bg-background max-w-sm rounded-lg border p-6 shadow-lg" onClick={e => e.stopPropagation()}>
                            <h3 className="mb-4 text-sm font-semibold">Keyboard Shortcuts</h3>
                            <div className="space-y-2 text-xs">
                                {[
                                    ["Click", "Select node & show details"],
                                    ["Double-click", "Focus mode (dim beyond 2 hops)"],
                                    ["Shift+Click", "Multi-select nodes"],
                                    ["Alt+Click", "Path finding mode"],
                                    ["Tab / Shift+Tab", "Cycle connections"],
                                    ["Escape", "Deselect / close"],
                                    ["Drag node", "Reposition"],
                                    ["?", "Toggle this help"]
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

                {/* Top-left: Filter panel (reuses the dialog's form) */}
                <div className="max-md:hidden">
                    <GraphFilters
                        filter={prefilter}
                        onFilterChange={handleFilterApply}
                        previewData={
                            data?.chunks
                                ? {
                                      chunks: data.chunks,
                                      connections: data.connections ?? [],
                                      chunkTags: scopedChunkTags ?? []
                                  }
                                : undefined
                        }
                        edgeAnimated={edgeAnimated}
                        onToggleEdgeAnimated={() => dispatch({ type: "TOGGLE_EDGE_ANIMATED" })}
                        showUngrouped={showUngrouped}
                        onToggleUngrouped={() => dispatch({ type: "TOGGLE_UNGROUPED" })}
                        hasActiveGrouping={activeTagTypeIds.size > 0 || prefilter.groupBy === "type" || prefilter.groupBy === "codebase"}
                        activeTypes={filterTypes}
                        activeRelations={filterRelations}
                        activeTagTypeIds={activeTagTypeIds}
                        availableTagTypeIds={availableTagTypeIds}
                        onApplyPreset={(filters) => {
                            dispatch({ type: "SET_FILTER_TYPES", types: new Set(filters.activeTypes) });
                            dispatch({ type: "SET_FILTER_RELATIONS", relations: new Set(filters.activeRelations) });
                            dispatch({ type: "SET_ACTIVE_TAG_TYPE_IDS", ids: new Set(filters.activeTagTypeIds) });
                        }}
                    />
                </div>

                {/* Top-right: Search + Stats */}
                <div className="absolute top-4 right-4 z-10 flex items-center gap-2">
                    <div className="relative flex items-center">
                        <input
                            type="text"
                            value={searchQuery}
                            onChange={e => dispatch({ type: "SET_SEARCH_QUERY", query: e.target.value })}
                            placeholder="Search nodes..."
                            className="bg-background/80 focus:ring-ring w-48 rounded-md border px-2.5 py-1.5 pr-7 text-xs backdrop-blur-sm focus:ring-2 focus:outline-none"
                        />
                        {searchQuery && (
                            <button
                                type="button"
                                onClick={() => dispatch({ type: "SET_SEARCH_QUERY", query: "" })}
                                className="text-muted-foreground hover:text-foreground absolute right-1.5 text-xs"
                                aria-label="Clear search"
                            >
                                &times;
                            </button>
                        )}
                    </div>
                    {debouncedSearchQuery.trim() && (
                        <span className="bg-background/80 text-muted-foreground whitespace-nowrap rounded-md border px-2 py-1.5 text-[10px] backdrop-blur-sm">
                            {searchMatchIds.size} of {layoutNodes.filter(n => !isGroupNodeId(n.id) && n.type !== "cluster").length}
                        </span>
                    )}
                    <Popover open={showPathPanel} onOpenChange={(v) => dispatch({ type: "SET_SHOW_PATH_PANEL", show: v })}>
                        <PopoverTrigger
                            className={`rounded-md border p-1.5 backdrop-blur-sm ${
                                showPathPanel || pathStartId
                                    ? "bg-primary text-primary-foreground"
                                    : "bg-background/80 text-muted-foreground hover:text-foreground"
                            }`}
                        >
                            <Route className="size-4" />
                        </PopoverTrigger>
                        <PopoverPopup side="bottom" align="end" sideOffset={8} className="w-72">
                            <PathPanel
                                chunks={(data?.chunks ?? []).map(c => ({ id: c.id, title: c.title }))}
                                pathStartId={pathStartId}
                                pathEndId={pathEndId}
                                pathResult={pathResult}
                                edges={layoutEdges.map(e => ({ id: e.id, source: e.source, target: e.target, data: e.data as { relation?: string } | undefined }))}
                                onSetStart={(id) => dispatch({ type: "SET_PATH_START", id })}
                                onSetEnd={(id) => dispatch({ type: "SET_PATH_END", id })}
                                onClear={() => dispatch({ type: "CLEAR_PATH" })}
                            />
                        </PopoverPopup>
                    </Popover>
                    <GraphSettingsPanel
                        layoutAlgorithm={layoutAlgorithm}
                        onLayoutChange={(algorithm) => dispatch({ type: "SET_LAYOUT_ALGORITHM", algorithm })}
                        hasDraggedPositions={draggedPositions.size > 0}
                        onResetLayout={() => setDraggedPositions(new Map())}
                        exploreMode={exploreMode}
                        exploredNodeIds={exploredNodeIds}
                        onToggleExploreMode={() => {
                            if (!exploreMode) {
                                dispatch({ type: "SET_EXPLORE_MODE", enabled: true });
                                dispatch({ type: "SET_EXPLORED_NODE_IDS", ids: selectedChunkId ? new Set([selectedChunkId]) : new Set() });
                            } else {
                                dispatch({ type: "SET_EXPLORE_MODE", enabled: false });
                            }
                        }}
                        onResetExplored={() => dispatch({ type: "SET_EXPLORED_NODE_IDS", ids: new Set() })}
                        bundleEdges={bundleEdges}
                        onToggleBundleEdges={() => dispatch({ type: "TOGGLE_BUNDLE_EDGES" })}
                        useMainThread={useMainThread}
                        onToggleMainThread={() => dispatch({ type: "TOGGLE_USE_MAIN_THREAD" })}
                        onSaveView={() => dispatch({ type: "SET_SHOW_SAVE_DIALOG", show: true })}
                        savedViews={savedViews}
                        onRestoreView={(view) => dispatch({
                            type: "RESTORE_VIEW",
                            filterTypes: view.filterTypes,
                            filterRelations: view.filterRelations,
                            collapsedParents: view.collapsedParents,
                            layoutAlgorithm: view.layoutAlgorithm as LayoutAlgorithm,
                            focusNodeId: view.focusNodeId,
                        })}
                        onDeleteView={deleteSavedView}
                        onSaveCustomGraph={() => setShowSaveCustomDialog(true)}
                        savedGraphs={Array.isArray(savedGraphsQuery.data) ? savedGraphsQuery.data : []}
                        onOpenGraph={(id) => navigate({ to: "/graph/$graphId", params: { graphId: id } })}
                        onDeleteGraph={(id) => deleteCustomGraphMutation.mutate(id)}
                        onExportImage={handleExportImage}
                        onExportMermaid={() => setMermaidModalOpen(true)}
                    />
                    <span className="text-muted-foreground bg-background/80 rounded-lg border px-3 py-1.5 text-xs backdrop-blur-sm">
                        {nodes.length - 1} · {edges.length}
                    </span>
                </div>

                {/* Top-center: Legend (catalog-driven, live counts, filter toggles) */}
                <GraphLegend
                    typeCounts={legendTypeCounts}
                    activeTypePrefilter={new Set(prefilter.types)}
                    onToggleTypePrefilter={toggleTypePrefilterFromLegend}
                    relationCounts={legendRelationCounts}
                    activeRelationFilter={filterRelations}
                    onToggleRelationFilter={toggleRelation}
                />

                {/* Tooltip */}
                {hoveredNode &&
                    chunkMap.get(hoveredNode.id) &&
                    (() => {
                        const info = chunkMap.get(hoveredNode.id)!;
                        return (
                            <div
                                className="bg-popover text-popover-foreground pointer-events-none fixed z-50 max-w-xs rounded-lg border p-3 shadow-lg"
                                style={{ left: hoveredNode.x + 12, top: hoveredNode.y + 12 }}
                            >
                                <p className="text-sm font-semibold">{info.title}</p>
                                <p className="text-muted-foreground mt-0.5 text-xs">{info.type}</p>
                                {info.summary && <p className="mt-1.5 text-xs">{info.summary}</p>}
                                {info.tags.length > 0 && (
                                    <div className="mt-1.5 flex flex-wrap gap-1">
                                        {info.tags.slice(0, 5).map(tag => (
                                            <span key={tag} className="bg-muted rounded px-1.5 py-0.5 text-[10px]">
                                                {tag}
                                            </span>
                                        ))}
                                    </div>
                                )}
                            </div>
                        );
                    })()}
                {/* Change type dialog (opened from quick-connect toast) */}
                <ChangeConnectionDialog
                    pendingConnection={pendingConnection}
                    chunkMap={chunkMap}
                    createConnectionMutation={createConnectionMutation}
                    dispatch={dispatch}
                />

                {/* Bottom-center: Timeline */}
                <GraphTimeline chunks={data?.chunks ?? []} onCutoffChange={handleTimelineCutoff} />

                {/* Bottom-left: Metrics */}
                <GraphMetrics
                    nodes={nodes}
                    edges={edges}
                    onNodeClick={id => dispatch({ type: "SELECT_AND_FOCUS_NODE", id })}
                />

                {/* Focus mode indicator */}
                {focusModeNodeId && (
                    <div className="absolute bottom-16 left-1/2 z-10 -translate-x-1/2 rounded-full border bg-background/90 px-4 py-1.5 text-xs shadow-sm backdrop-blur-sm">
                        Focus mode — click node again or press <kbd className="mx-1 rounded border px-1.5 py-0.5 font-mono">Esc</kbd> to exit
                    </div>
                )}

                {/* Bulk action bar */}
                {multiSelectedIds.size > 0 && (
                    <div className="bg-background/95 absolute bottom-4 left-1/2 z-20 flex -translate-x-1/2 items-center gap-2 rounded-lg border px-4 py-2 shadow-lg backdrop-blur-sm">
                        <span className="text-xs font-medium">{multiSelectedIds.size} selected</span>
                        <div className="bg-border h-4 w-px" />
                        <button
                            onClick={() => dispatch({ type: "SET_SHOW_DELETE_CONFIRM", show: true })}
                            className="text-destructive hover:bg-destructive/10 rounded-md px-2 py-1 text-xs"
                        >
                            Delete
                        </button>
                        <button
                            onClick={() => dispatch({ type: "CLEAR_MULTI_SELECT" })}
                            className="text-muted-foreground hover:text-foreground rounded-md px-2 py-1 text-xs"
                        >
                            Clear
                        </button>
                    </div>
                )}

                {/* Path info bar */}
                {(pathStartId || pathEndId) && (
                    <div className="absolute bottom-6 left-1/2 z-10 -translate-x-1/2">
                        <div className="bg-background/90 flex items-center gap-3 rounded-lg border px-4 py-2 text-sm shadow-lg backdrop-blur-sm">
                            {pathStartId && !pathEndId && (
                                <span className="text-muted-foreground">
                                    Alt+click another node to find path from{" "}
                                    <span className="text-foreground font-medium">{chunkMap.get(pathStartId)?.title ?? pathStartId}</span>
                                </span>
                            )}
                            {pathStartId && pathEndId && pathResult && (
                                <span className="text-foreground font-medium">
                                    Path: {pathResult.length} {pathResult.length === 1 ? "hop" : "hops"}
                                </span>
                            )}
                            {pathStartId && pathEndId && !pathResult && <span className="font-medium text-red-500">No path found</span>}
                            <button
                                onClick={() => dispatch({ type: "CLEAR_PATH" })}
                                className="text-muted-foreground hover:text-foreground rounded border px-2 py-0.5 text-xs"
                            >
                                Clear
                            </button>
                        </div>
                    </div>
                )}
                {/* Save view dialog */}
                <SaveViewDialog
                    show={showSaveDialog}
                    viewName={viewName}
                    filterTypes={filterTypes}
                    filterRelations={filterRelations}
                    collapsedParents={collapsedParents}
                    layoutAlgorithm={layoutAlgorithm}
                    focusedNodeId={focusedNodeId}
                    saveView={saveView}
                    dispatch={dispatch}
                />
                {/* Save custom graph dialog */}
                <SaveCustomGraphDialog
                    show={showSaveCustomDialog}
                    onClose={() => setShowSaveCustomDialog(false)}
                    customGraphName={customGraphName}
                    onNameChange={setCustomGraphName}
                    visibleChunkCount={filteredGraph?.chunks.length ?? 0}
                    filteredChunkIds={filteredGraph?.chunks.map(c => c.id) ?? []}
                    draggedPositions={draggedPositions}
                    layoutPositions={layoutPositions}
                    layoutAlgorithm={layoutAlgorithm}
                    codebaseId={codebaseId}
                    saveCustomGraphMutation={saveCustomGraphMutation}
                />
            </div>

            <ConfirmDialog
                open={showDeleteConfirm}
                onOpenChange={(v) => dispatch({ type: "SET_SHOW_DELETE_CONFIRM", show: v })}
                title="Delete chunks"
                description={`Delete ${multiSelectedIds.size} chunks?`}
                confirmLabel="Delete"
                confirmVariant="destructive"
                onConfirm={() => {
                    deleteManyMutation.mutate([...multiSelectedIds]);
                    dispatch({ type: "SET_SHOW_DELETE_CONFIRM", show: false });
                }}
                loading={deleteManyMutation.isPending}
            />

            <MermaidExportModal
                open={mermaidModalOpen}
                onOpenChange={setMermaidModalOpen}
                nodes={nodes}
                edges={edges}
            />

            <GraphFilterDialog
                open={filterDialogOpen}
                onOpenChange={setFilterDialogOpen}
                initial={prefilter}
                previewData={
                    data?.chunks
                        ? {
                              chunks: data.chunks,
                              connections: data.connections ?? [],
                              chunkTags: scopedChunkTags ?? []
                          }
                        : undefined
                }
                onApply={handleFilterApply}
                onShowEverything={handleShowEverything}
                availableTagTypeIds={availableTagTypeIds}
            />
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
