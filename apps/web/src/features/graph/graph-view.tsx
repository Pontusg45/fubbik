import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useSearch } from "@tanstack/react-router";
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
import { Filter, Route } from "lucide-react";
import { useTheme } from "next-themes";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Popover, PopoverTrigger, PopoverPopup } from "@/components/ui/popover";
import { Sheet, SheetContent } from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { relationColor } from "@/features/chunks/relation-colors";
import { FloatingEdge } from "@/features/graph/floating-edge";
import { runForceLayout } from "@/features/graph/force-layout";
import { GraphDetailPanel } from "@/features/graph/graph-detail-panel";
import { GraphFilters } from "@/features/graph/graph-filters";
import { GraphSettingsPanel } from "@/features/graph/graph-settings-panel";
import { GraphFilterDialog, EMPTY_FILTER, type GraphFilterValues } from "@/features/graph/graph-filter-dialog";
import { GROUP_NODE_ID_PREFIX, GROUP_STRATEGIES, UNGROUPED_NODE_ID, isGroupNodeId, type GroupBy } from "@/features/graph/group-strategies";
import { applyPrefilter } from "@/features/graph/apply-prefilter";
import { MermaidExportModal } from "@/features/graph/mermaid-export-modal";
import { GraphLegend } from "@/features/graph/graph-legend";
import { GraphMetrics } from "@/features/graph/graph-metrics";
import { GraphNode } from "@/features/graph/graph-node";
import { GraphGroupNode } from "@/features/graph/graph-group-node";
import { GraphContextMenu } from "@/features/graph/graph-context-menu";
import { GraphWelcome } from "@/features/graph/graph-welcome";
import type { LayoutWorkerInput, LayoutWorkerOutput } from "@/features/graph/layout.worker";
import { type LayoutAlgorithm, hierarchicalLayout, radialLayout } from "@/features/graph/layouts";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { ChangeConnectionDialog, SaveViewDialog, SaveCustomGraphDialog } from "./graph-dialogs";
import { findShortestPath, getNodesWithinHops, getMostConnected } from "./graph-utils";
import { GraphTimeline } from "./graph-timeline";
import { PathPanel } from "./path-panel";
import { useGraphKeyboard } from "./use-graph-keyboard";
import { useGraphState } from "./use-graph-state";
import { useSavedGraphViews } from "./use-saved-views";




const EDGE_TYPES = { floating: FloatingEdge };
const NODE_TYPES = { chunk: GraphNode, group: GraphGroupNode };

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
        // Show relation picker dialog before creating the connection
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

    const { codebaseId, workspaceId } = useActiveCodebase();

    const { data, isLoading } = useQuery({
        queryKey: ["graph", codebaseId, workspaceId],
        queryFn: async () => {
            return unwrapEden(
                await api.api.graph.get({
                    query: {
                        ...(workspaceId ? { workspaceId } : {}),
                        ...(codebaseId && codebaseId !== "global" && !workspaceId ? { codebaseId } : {})
                    }
                })
            );
        }
    });

    const debouncedSearchQuery = useDebouncedValue(searchQuery, 150);

    // Measured node sizes for group bounding box calculation (ref + counter to trigger recalc)
    const measuredNodeSizesRef = useRef(new Map<string, { w: number; h: number }>());
    const [measuredSizesVersion, setMeasuredSizesVersion] = useState(0);

    // Read path params from URL search
    const search = useSearch({ strict: false }) as {
        pathFrom?: string;
        pathTo?: string;
        tags?: string;
        types?: string;
        focus?: string;
        depth?: number;
        groupBy?: "tag" | "type" | "codebase" | "none";
        all?: number;
    };
    useEffect(() => {
        if (search.pathFrom) {
            dispatch({ type: "SET_PATH_START", id: search.pathFrom });
            dispatch({ type: "SET_SHOW_PATH_PANEL", show: true });
        }
        if (search.pathTo) {
            dispatch({ type: "SET_PATH_END", id: search.pathTo });
            dispatch({ type: "SET_SHOW_PATH_PANEL", show: true });
        }
    }, [search.pathFrom, search.pathTo, dispatch]);

    // Pre-filter driven by URL params (set via GraphFilterDialog on entry)
    const prefilter = useMemo<GraphFilterValues>(() => {
        if (!search.tags && !search.types && !search.focus && !search.groupBy) return EMPTY_FILTER;
        return {
            tags: search.tags ? search.tags.split(",").filter(Boolean) : [],
            types: search.types ? search.types.split(",").filter(Boolean) : [],
            focusChunkId: search.focus ?? null,
            depth: typeof search.depth === "number" && search.depth >= 1 && search.depth <= 3 ? search.depth : 2,
            groupBy: search.groupBy ?? "tag"
        };
    }, [search.tags, search.types, search.focus, search.depth, search.groupBy]);

    const hasAnyFilterParams = !!(search.tags || search.types || search.focus || search.groupBy || search.all);
    const [filterDialogOpen, setFilterDialogOpen] = useState(!hasAnyFilterParams);

    useEffect(() => {
        // Re-open dialog if user navigates back to /graph with no params (e.g., clear)
        if (!hasAnyFilterParams) setFilterDialogOpen(true);
    }, [hasAnyFilterParams]);

    // Apply groupBy from prefilter once graph data is available
    useEffect(() => {
        if (!data?.tagTypes) return;
        if (prefilter.groupBy === "tag") {
            const ids = new Set(data.tagTypes.map(tt => tt.id));
            if (ids.size > 0) dispatch({ type: "SET_ACTIVE_TAG_TYPE_IDS", ids });
        } else if (prefilter.groupBy === "none") {
            dispatch({ type: "SET_ACTIVE_TAG_TYPE_IDS", ids: new Set() });
        }
        // "type" grouping is handled via the filteredGraph pre-filter (types narrow the set);
        // visual grouping-by-type is a future addition.
    }, [prefilter.groupBy, data?.tagTypes, dispatch]);

    useEffect(() => {
        if (typeof window !== "undefined" && !localStorage.getItem("fubbik-graph-welcomed")) {
            dispatch({ type: "SET_SHOW_WELCOME", show: true });
        }
    }, [dispatch]);

    const dismissWelcome = () => {
        dispatch({ type: "SET_SHOW_WELCOME", show: false });
        localStorage.setItem("fubbik-graph-welcomed", "true");
    };

    // Saved views
    const { views: savedViews, saveView, deleteView: deleteSavedView } = useSavedGraphViews();

    // Dragged node positions (persist across layout changes)
    const [draggedPositions, setDraggedPositions] = useState<Map<string, { x: number; y: number }>>(new Map());
    const groupDragStartRef = useRef<{ groupId: string; startPos: { x: number; y: number } } | null>(null);

    // Active types/relations for legend
    const activeTypes = useMemo(() => new Set((data?.chunks ?? []).map(c => c.type)), [data]);
    const activeRelations = useMemo(() => new Set((data?.connections ?? []).map(c => c.relation)), [data]);

    // Toggle helpers
    function toggleType(t: string) {
        dispatch({ type: "TOGGLE_FILTER_TYPE", filterType: t });
    }
    function toggleRelation(r: string) {
        dispatch({ type: "TOGGLE_FILTER_RELATION", relation: r });
    }

    // Resolve active grouping strategy from prefilter + existing tag-type toggles.
    // prefilter.groupBy explicitly wins when set to something other than "tag" (its default).
    // "tag" falls back to activeTagTypeIds (the in-graph toggle).
    const groupingMode: Exclude<GroupBy, "none"> | null = useMemo(() => {
        if (prefilter.groupBy === "type") return "type";
        if (prefilter.groupBy === "codebase") return "codebase";
        if (prefilter.groupBy === "none") return null;
        if (activeTagTypeIds.size > 0) return "tag";
        return null;
    }, [prefilter.groupBy, activeTagTypeIds]);

    const groupResult = useMemo(() => {
        if (!groupingMode || !data) return null;
        const typeColorMap: Record<string, string> = {};
        for (const [name, palette] of Object.entries(TYPE_COLORS)) typeColorMap[name] = palette.border;
        return GROUP_STRATEGIES[groupingMode].build({
            chunks: data.chunks,
            chunkTags: data.chunkTags,
            activeTagTypeIds,
            chunkCodebases: data.chunkCodebases,
            typeColorMap
        });
    }, [groupingMode, data, activeTagTypeIds, TYPE_COLORS]);

    // Keep `tagGroups` variable name — downstream pipeline references it by this name.
    const tagGroups = groupResult?.groups ?? null;

    // Build chunk-to-tag-group lookup for edge opacity
    const chunkTagGroupMap = useMemo(() => {
        if (!tagGroups) return null;
        const map = new Map<string, Set<string>>();
        for (const [tagName, chunkIds] of tagGroups) {
            for (const cid of chunkIds) {
                if (!map.has(cid)) map.set(cid, new Set());
                map.get(cid)!.add(tagName);
            }
        }
        return map;
    }, [tagGroups]);

    // --- Web Worker for force-directed layout ---
    const workerRef = useRef<Worker | null>(null);
    const requestIdRef = useRef<number>(0);
    const [layoutPositions, setLayoutPositions] = useState<Record<string, { x: number; y: number }> | null>(null);
    const [isLayouting, setIsLayouting] = useState(false);

    // Create / teardown the worker
    useEffect(() => {
        const worker = new Worker(new URL("./layout.worker.ts", import.meta.url), { type: "module" });
        worker.onmessage = (e: MessageEvent<LayoutWorkerOutput>) => {
            if (e.data.requestId !== requestIdRef.current) return;
            setLayoutPositions(e.data.positions);
            setIsLayouting(false);
        };
        worker.onerror = err => {
            console.error("Layout worker error:", err);
            setIsLayouting(false);
        };
        workerRef.current = worker;
        return () => {
            worker.terminate();
            workerRef.current = null;
        };
    }, []);

    // Pre-compute filtered data that both the worker and the styling memo need
    const filteredGraph = useMemo(() => {
        if (!data?.chunks || data.chunks.length === 0) return null;

        // Shared prefilter logic (see apply-prefilter.ts) — same function as the dialog preview.
        const prefiltered = applyPrefilter(
            { chunks: data.chunks, connections: data.connections ?? [], chunkTags: data.chunkTags },
            { tags: prefilter.tags, types: prefilter.types, focusChunkId: prefilter.focusChunkId, depth: prefilter.depth }
        );
        let chunks = prefiltered.chunks;
        let connections = prefiltered.connections;

        // Build parent-children map from part_of edges
        const parentChildren = new Map<string, Set<string>>();
        for (const conn of connections) {
            if (conn.relation === "part_of") {
                if (!parentChildren.has(conn.targetId)) parentChildren.set(conn.targetId, new Set());
                parentChildren.get(conn.targetId)!.add(conn.sourceId);
            }
        }

        // Collect all child IDs (nodes that are part_of a parent)
        const childIds = new Set<string>();
        for (const children of parentChildren.values()) {
            for (const childId of children) childIds.add(childId);
        }

        // Hide children of collapsed parents
        const hiddenIds = new Set<string>();
        for (const [parentId, children] of parentChildren) {
            if (collapsedParents.has(parentId)) {
                for (const childId of children) hiddenIds.add(childId);
            }
        }

        // Apply type filter
        if (filterTypes.size > 0) {
            chunks = chunks.filter(c => filterTypes.has(c.type));
        }

        // Apply relation filter
        if (filterRelations.size > 0) {
            connections = connections.filter(c => filterRelations.has(c.relation));
        }

        // Remove hidden chunks
        const visibleChunkIds = new Set(chunks.filter(c => !hiddenIds.has(c.id)).map(c => c.id));

        // Filter connections to only visible chunks
        connections = connections.filter(c => visibleChunkIds.has(c.sourceId) && visibleChunkIds.has(c.targetId));

        // Explore mode: only show explored nodes
        if (exploreMode && exploredNodeIds.size > 0) {
            chunks = chunks.filter(c => exploredNodeIds.has(c.id));
            connections = connections.filter(c => exploredNodeIds.has(c.sourceId) && exploredNodeIds.has(c.targetId));
        }

        // Timeline filter
        if (timelineCutoff) {
            chunks = chunks.filter(c => new Date(c.createdAt) <= timelineCutoff);
        }

        return { chunks, connections, parentChildren, childIds, hiddenIds };
    }, [data, filterTypes, filterRelations, collapsedParents, exploreMode, exploredNodeIds, timelineCutoff, prefilter]);

    // Post to worker when simulation inputs change
    useEffect(() => {
        if (!filteredGraph) return;

        const { chunks, connections, hiddenIds } = filteredGraph;

        // Build worker input: nodes need id and type for clustering
        const workerNodes: LayoutWorkerInput["nodes"] = [
            ...chunks.filter(c => !hiddenIds.has(c.id)).map(c => ({ id: c.id, type: c.type }))
        ];

        const workerEdges: LayoutWorkerInput["edges"] = connections.map(conn => ({
            source: conn.sourceId,
            target: conn.targetId,
            relation: conn.relation
        }));

        if (layoutAlgorithm === "force") {
            const tagGroupsObj = tagGroups ? Object.fromEntries(tagGroups) : undefined;
            if (useMainThread) {
                setIsLayouting(true);
                // Run synchronously on main thread — avoids worker serialization overhead
                const positions = runForceLayout(workerNodes, workerEdges, tagGroups ?? undefined);
                setLayoutPositions(positions);
                setIsLayouting(false);
            } else {
                if (!workerRef.current) return;
                setIsLayouting(true);
                requestIdRef.current += 1;
                workerRef.current.postMessage({
                    requestId: requestIdRef.current,
                    nodes: workerNodes,
                    edges: workerEdges,
                    tagGroups: tagGroupsObj
                } satisfies LayoutWorkerInput);
            }
        } else {
            let positions: Record<string, { x: number; y: number }>;
            if (layoutAlgorithm === "hierarchical") {
                positions = hierarchicalLayout(workerNodes, workerEdges);
            } else {
                // Radial: use selected node or most-connected as center
                const center = selectedChunkId ?? getMostConnected(workerNodes, workerEdges);
                positions = center ? radialLayout(workerNodes, workerEdges, center) : hierarchicalLayout(workerNodes, workerEdges);
            }
            setLayoutPositions(positions);
            setIsLayouting(false);
        }
    }, [filteredGraph, layoutAlgorithm, useMainThread, selectedChunkId, tagGroups]);

    // Build layoutNodes and layoutEdges from positions (cheap: styling + edge creation only)
    const { layoutNodes, layoutEdges, groupToChunkIds } = useMemo(() => {
        if (!filteredGraph) return { layoutNodes: [] as Node[], layoutEdges: [] as Edge[], groupToChunkIds: new Map<string, string[]>() };

        const { chunks, connections, parentChildren, childIds, hiddenIds } = filteredGraph;

        // Build chunk→codebaseId map for cross-codebase edge styling + node labels
        const chunkCodebaseMap = new Map<string, string>();
        for (const cc of data?.chunkCodebases ?? []) {
            if (!chunkCodebaseMap.has(cc.chunkId)) {
                chunkCodebaseMap.set(cc.chunkId, cc.codebaseId);
            }
        }

        // Connection counts for node sizing
        const connectionCounts = new Map<string, number>();
        for (const conn of connections) {
            connectionCounts.set(conn.sourceId, (connectionCounts.get(conn.sourceId) ?? 0) + 1);
            connectionCounts.set(conn.targetId, (connectionCounts.get(conn.targetId) ?? 0) + 1);
        }

        let rawNodes: Node[] = [
            ...chunks
                .filter(c => !hiddenIds.has(c.id))
                .map(c => {
                    const typeColor = TYPE_COLORS[c.type] ?? TYPE_COLORS.note;
                    const count = connectionCounts.get(c.id) ?? 0;
                    const isParent = parentChildren.has(c.id);
                    const isChild = childIds.has(c.id);
                    const childCount = collapsedParents.has(c.id) ? (parentChildren.get(c.id)?.size ?? 0) : 0;
                    const label = childCount > 0 ? `${c.title} (${childCount})` : c.title;
                    return {
                        id: c.id,
                        type: "chunk",
                        data: {
                            label,
                            type: c.type,
                            connectionCount: count,
                            tags: [] as string[],
                            ...(chunkCodebaseMap.size > 0 && chunkCodebaseMap.has(c.id)
                                ? { codebaseName: (data?.chunkCodebases ?? []).find(cc => cc.chunkId === c.id)?.codebaseName }
                                : {})
                        },
                        position: { x: 0, y: 0 },
                        style: {
                            cursor: "pointer",
                            background: typeColor!.bg,
                            borderColor: typeColor!.border,
                            borderWidth: isParent ? 2 : collapsedParents.has(c.id) ? 2.5 : 1.5,
                            borderRadius: isParent ? 12 : 10,
                            color: isDark ? (isChild ? "#cbd5e1" : "#e2e8f0") : isChild ? "#475569" : "#1e293b",
                            fontSize: isParent ? 13 : 12,
                            fontWeight: isParent ? 600 : 500,
                            padding: isParent ? "7px 12px" : "5px 10px",
                            whiteSpace: "nowrap" as const,
                            overflow: "hidden" as const,
                            textOverflow: "ellipsis" as const,
                            maxWidth: 250,
                            letterSpacing: "0.01em"
                        }
                    };
                })
        ];

        const rawEdges: Edge[] = connections.map(conn => {
            const color = relationColor(conn.relation);
            const sourceCb = chunkCodebaseMap.get(conn.sourceId);
            const targetCb = chunkCodebaseMap.get(conn.targetId);
            const isCrossCodebase = sourceCb && targetCb && sourceCb !== targetCb;
            return {
                id: conn.id,
                source: conn.sourceId,
                target: conn.targetId,
                type: "floating",
                data: { relation: conn.relation, directed: true },
                animated: edgeAnimated,
                style: {
                    stroke: color,
                    strokeWidth: 2,
                    transition: "opacity 0.3s ease",
                    ...(isCrossCodebase ? { strokeDasharray: "6 3" } : {})
                }
            };
        });

        // Detect parallel edges between same node pairs
        const edgeKey = (a: string, b: string) => [a, b].sort().join("::");
        const parallelCounts = new Map<string, number>();
        for (const edge of rawEdges) {
            const key = edgeKey(edge.source, edge.target);
            parallelCounts.set(key, (parallelCounts.get(key) ?? 0) + 1);
        }
        const parallelIndex = new Map<string, number>();
        for (const edge of rawEdges) {
            const key = edgeKey(edge.source, edge.target);
            const total = parallelCounts.get(key) ?? 1;
            if (total <= 1) continue;
            const idx = parallelIndex.get(key) ?? 0;
            parallelIndex.set(key, idx + 1);
            (edge.data as Record<string, unknown>).curveOffset = (idx - (total - 1) / 2) * 40;
        }

        // Build tag group nodes (visual overlay — no parentId needed)
        const tagGroupNodeIds = new Set<string>();
        const chunkToGroupId = new Map<string, string>(); // chunkId -> groupNodeId

        if (tagGroups && tagGroups.size > 0) {
            const colorFor = groupResult?.colorFor;

            // Build set of visible chunk IDs (nodes already in rawNodes)
            const visibleChunkIds = new Set(rawNodes.map(n => n.id));

            for (const [label, chunkIds] of tagGroups) {
                // Only include chunks that are actually visible (not filtered out)
                const visibleInGroup = chunkIds.filter(cid => visibleChunkIds.has(cid));
                if (visibleInGroup.length === 0) continue; // skip empty groups

                const groupId = `${GROUP_NODE_ID_PREFIX}${label}`;
                tagGroupNodeIds.add(groupId);
                for (const cid of visibleInGroup) {
                    if (!chunkToGroupId.has(cid)) {
                        chunkToGroupId.set(cid, groupId);
                    }
                }
                const color = colorFor?.(label) ?? "#8b5cf6";
                rawNodes.unshift({
                    id: groupId,
                    type: "group",
                    data: { label, color },
                    position: { x: 0, y: 0 },
                    selectable: false,
                    draggable: true,
                    style: { zIndex: -1, background: "transparent", border: "none", padding: 0 }
                });
            }

            // Add "ungrouped" group for chunks without a tag in this type
            const groupedChunkIds = new Set(chunkToGroupId.keys());
            const ungroupedChunkIds = new Set(
                rawNodes.filter(n => !tagGroupNodeIds.has(n.id) && !groupedChunkIds.has(n.id)).map(n => n.id)
            );
            if (!showUngrouped && ungroupedChunkIds.size > 0) {
                // Remove ungrouped chunks from the graph entirely
                rawNodes = rawNodes.filter(n => !ungroupedChunkIds.has(n.id));
            }
            if (showUngrouped && ungroupedChunkIds.size > 0) {
                const ungroupedChunks = rawNodes.filter(n => ungroupedChunkIds.has(n.id));
                const ungroupedId = UNGROUPED_NODE_ID;
                tagGroupNodeIds.add(ungroupedId);
                for (const c of ungroupedChunks) {
                    chunkToGroupId.set(c.id, ungroupedId);
                }
                rawNodes.unshift({
                    id: ungroupedId,
                    type: "group",
                    data: { label: "ungrouped", color: isDark ? "#475569" : "#94a3b8" },
                    position: { x: 0, y: 0 },
                    selectable: false,
                    draggable: true,
                    style: { zIndex: -1, background: "transparent", border: "none", padding: 0 }
                });
            }
        }

        // Build reverse map: groupId -> chunkIds
        const groupToChunkIds = new Map<string, string[]>();
        for (const [chunkId, groupId] of chunkToGroupId) {
            const arr = groupToChunkIds.get(groupId);
            if (arr) arr.push(chunkId);
            else groupToChunkIds.set(groupId, [chunkId]);
        }

        // Remove group nodes that ended up with zero children
        const emptyGroupIds = new Set<string>();
        for (const groupId of tagGroupNodeIds) {
            const children = groupToChunkIds.get(groupId);
            if (!children || children.length === 0) {
                emptyGroupIds.add(groupId);
            }
        }
        if (emptyGroupIds.size > 0) {
            rawNodes = rawNodes.filter(n => !emptyGroupIds.has(n.id));
            for (const id of emptyGroupIds) tagGroupNodeIds.delete(id);
        }

        // Pre-compute group bounding boxes from child layout positions
        const PADDING = 14;
        const PADDING_TOP = 40;
        const PADDING_BOTTOM = 28;
        const DEFAULT_NODE_W = 180;
        const DEFAULT_NODE_H = 36;
        const GRID_GAP_X = 16; // horizontal gap between columns
        const GRID_GAP_Y = 12; // vertical gap between rows
        const groupBounds = new Map<string, { minX: number; minY: number; maxX: number; maxY: number }>();

        const measuredSizes = measuredNodeSizesRef.current;

        // Re-space grid positions using measured node widths/heights. The worker
        // lays chunks on a uniform grid (NODE_W=200); long titles can exceed this
        // and cause siblings to overlap. For each group we compute a per-column
        // max-width and per-row max-height, then re-emit each chunk's center at
        // the cumulative offset from the group's centroid.
        //
        // This runs client-side where measured DOM sizes are available; the worker
        // only provides the rough grid order, which we preserve.
        const respacedPositions = new Map<string, { x: number; y: number }>();
        if (tagGroupNodeIds.size > 0 && layoutPositions) {
            // Build per-group member lists in chunkToGroupId insertion order — same
            // order the worker used to populate the grid.
            const groupMembers = new Map<string, string[]>();
            for (const [chunkId, groupId] of chunkToGroupId) {
                if (!groupMembers.has(groupId)) groupMembers.set(groupId, []);
                groupMembers.get(groupId)!.push(chunkId);
            }

            function colsForCount(n: number) {
                if (n <= 1) return 1;
                // Same aspect-balanced formula the worker uses, kept in sync so row
                // indices match the worker's row ordering when resolving ties.
                const NODE_W = 200, NODE_H = 72;
                return Math.max(1, Math.ceil(Math.sqrt(n * (NODE_H / NODE_W))));
            }

            for (const [, members] of groupMembers) {
                if (members.length === 0) continue;
                // Group centroid = average of worker positions; robust against any
                // member being dragged away (dragged ones take their own position).
                let cx = 0, cy = 0, centroidCount = 0;
                for (const cid of members) {
                    const lp = layoutPositions[cid];
                    if (lp) { cx += lp.x; cy += lp.y; centroidCount++; }
                }
                if (centroidCount === 0) continue;
                cx /= centroidCount;
                cy /= centroidCount;

                const cols = colsForCount(members.length);
                const rows = Math.ceil(members.length / cols);

                // Column widths: widest measured w in each column (fallback to default).
                // Row heights: same for each row.
                const colW: number[] = new Array(cols).fill(DEFAULT_NODE_W);
                const rowH: number[] = new Array(rows).fill(DEFAULT_NODE_H);
                for (let i = 0; i < members.length; i++) {
                    const cid = members[i]!;
                    const s = measuredSizes.get(cid);
                    const w = s?.w ?? DEFAULT_NODE_W;
                    const h = s?.h ?? DEFAULT_NODE_H;
                    const col = i % cols;
                    const row = Math.floor(i / cols);
                    if (w > colW[col]!) colW[col] = w;
                    if (h > rowH[row]!) rowH[row] = h;
                }

                // Cumulative column centers relative to the grid left edge.
                const colCenters: number[] = [];
                let xCursor = 0;
                for (let c = 0; c < cols; c++) {
                    colCenters.push(xCursor + colW[c]! / 2);
                    xCursor += colW[c]! + GRID_GAP_X;
                }
                const totalW = xCursor - GRID_GAP_X;

                const rowCenters: number[] = [];
                let yCursor = 0;
                for (let r = 0; r < rows; r++) {
                    rowCenters.push(yCursor + rowH[r]! / 2);
                    yCursor += rowH[r]! + GRID_GAP_Y;
                }
                const totalH = yCursor - GRID_GAP_Y;

                // Shift so the grid is centered on (cx, cy).
                const originX = cx - totalW / 2;
                const originY = cy - totalH / 2;

                for (let i = 0; i < members.length; i++) {
                    const cid = members[i]!;
                    const col = i % cols;
                    const row = Math.floor(i / cols);
                    respacedPositions.set(cid, {
                        x: originX + colCenters[col]!,
                        y: originY + rowCenters[row]!
                    });
                }
            }

            // Build bounds from the re-spaced positions so the group box wraps
            // them exactly (respacing may have shifted edges by a few px).
            for (const [chunkId, groupId] of chunkToGroupId) {
                const dragged = draggedPositions.get(chunkId);
                const re = respacedPositions.get(chunkId);
                const lp = layoutPositions[chunkId];
                if (!dragged && !re && !lp) continue;
                const size = measuredSizes.get(chunkId);
                const w = size?.w ?? DEFAULT_NODE_W;
                const h = size?.h ?? DEFAULT_NODE_H;
                // Centers: dragged positions are top-left; re-spaced + worker positions are centers.
                const centerX = dragged ? dragged.x + w / 2 : (re?.x ?? lp!.x);
                const centerY = dragged ? dragged.y + h / 2 : (re?.y ?? lp!.y);
                const x = centerX - w / 2;
                const y = centerY - h / 2;
                const prev = groupBounds.get(groupId);
                if (prev) {
                    prev.minX = Math.min(prev.minX, x);
                    prev.minY = Math.min(prev.minY, y);
                    prev.maxX = Math.max(prev.maxX, x + w);
                    prev.maxY = Math.max(prev.maxY, y + h);
                } else {
                    groupBounds.set(groupId, { minX: x, minY: y, maxX: x + w, maxY: y + h });
                }
            }
        }

        // Apply positions from worker (or fallback to origin)
        const layoutNodes = rawNodes.map(node => {
            const p = layoutPositions?.[node.id] ?? { x: 0, y: 0 };

            // Group nodes: compute size from child bounds. Hide groups with no positioned children.
            if (tagGroupNodeIds.has(node.id)) {
                const bounds = groupBounds.get(node.id);
                if (!bounds) {
                    // No children have positions yet — hide the group entirely
                    return { ...node, position: { x: -9999, y: -9999 }, style: { ...node.style, display: "none" } };
                }
                const dragged = draggedPositions.get(node.id);
                return {
                    ...node,
                    position: dragged ?? { x: bounds.minX - PADDING, y: bounds.minY - PADDING_TOP },
                    style: {
                        ...node.style,
                        width: bounds.maxX - bounds.minX + PADDING * 2,
                        height: bounds.maxY - bounds.minY + PADDING_BOTTOM + PADDING_TOP,
                        background: "transparent",
                        border: "none",
                        padding: 0
                    }
                };
            }

            const dragged = draggedPositions.get(node.id);
            if (dragged) return { ...node, position: dragged };

            const size = measuredSizes.get(node.id);
            const hw = (size?.w ?? DEFAULT_NODE_W) / 2;
            const hh = (size?.h ?? DEFAULT_NODE_H) / 2;
            // Prefer the re-spaced position (accounts for variable node widths) over
            // the worker's fixed-cell grid position.
            const re = respacedPositions.get(node.id);
            const center = re ?? p;
            return { ...node, position: { x: center.x - hw, y: center.y - hh } };
        });

        if (bundleEdges) {
            // Build a node-type lookup from the filtered chunks
            const nodeTypeMap = new Map<string, string>();
            for (const c of filteredGraph.chunks) {
                nodeTypeMap.set(c.id, c.type);
            }

            // Group edges by sorted source-type::target-type pair
            const bundles = new Map<string, typeof rawEdges>();
            const orphanEdges: typeof rawEdges = [];
            for (const edge of rawEdges) {
                if (edge.id.startsWith("main-")) {
                    orphanEdges.push(edge);
                    continue;
                }
                const st = nodeTypeMap.get(edge.source) ?? "?";
                const tt = nodeTypeMap.get(edge.target) ?? "?";
                const key = [st, tt].sort().join("::");
                if (!bundles.has(key)) bundles.set(key, []);
                bundles.get(key)!.push(edge);
            }

            // Replace bundles of 3+ with single thicker edge
            const bundled: typeof rawEdges = [];
            for (const [, group] of bundles) {
                if (group.length < 3) {
                    bundled.push(...group);
                } else {
                    const rep = { ...group[0]! };
                    rep.style = { ...(rep.style as Record<string, unknown>), strokeWidth: Math.min(2 + group.length, 8) };
                    (rep.data as Record<string, unknown>).bundleCount = group.length;
                    bundled.push(rep);
                }
            }
            bundled.push(...orphanEdges);

            return { layoutNodes, layoutEdges: bundled, groupToChunkIds };
        }

        return { layoutNodes, layoutEdges: rawEdges, groupToChunkIds };
    }, [filteredGraph, layoutPositions, draggedPositions, isDark, TYPE_COLORS, collapsedParents, bundleEdges, tagGroups, data, edgeAnimated, measuredSizesVersion, showUngrouped]);

    // Search match IDs — shared between styling effect, auto-center, and match count display
    const searchMatchIds = useMemo(() => {
        const q = debouncedSearchQuery.trim().toLowerCase();
        if (!q) return new Set<string>();
        const ids = new Set<string>();
        for (const node of layoutNodes) {
            const label = typeof node.data.label === "string" ? node.data.label : "";
            if (label.toLowerCase().includes(q)) ids.add(node.id);
        }
        return ids;
    }, [debouncedSearchQuery, layoutNodes]);

    // Auto-center on first search match
    useEffect(() => {
        if (searchMatchIds.size === 0) return;
        const firstMatchId = [...searchMatchIds][0];
        const matchNode = layoutNodes.find(n => n.id === firstMatchId);
        if (matchNode?.position) {
            setCenter(matchNode.position.x, matchNode.position.y, { zoom: getZoom(), duration: 400 });
        }
    }, [searchMatchIds, layoutNodes, setCenter, getZoom]);

    const focusNeighbors = useMemo(() => {
        if (!focusedNodeId) return null;
        const neighbors = new Set<string>([focusedNodeId]);
        for (const edge of layoutEdges) {
            if (edge.source === focusedNodeId) neighbors.add(edge.target);
            if (edge.target === focusedNodeId) neighbors.add(edge.source);
        }
        return neighbors;
    }, [focusedNodeId, layoutEdges]);

    // Focus mode: nodes within 2 hops of the focus mode node
    const focusModeNeighbors = useMemo(() => {
        if (!focusModeNodeId) return null;
        return getNodesWithinHops(focusModeNodeId, layoutEdges, 2);
    }, [focusModeNodeId, layoutEdges]);

    const selectedEdgeIds = useMemo(() => {
        if (!selectedChunkId) return null;
        const ids = new Set<string>();
        for (const edge of layoutEdges) {
            if (edge.source === selectedChunkId || edge.target === selectedChunkId) {
                ids.add(edge.id);
            }
        }
        return ids;
    }, [selectedChunkId, layoutEdges]);

    const selectedNeighborNodes = useMemo(() => {
        if (!selectedChunkId) return null;
        const neighbors = new Set<string>([selectedChunkId]);
        for (const edge of layoutEdges) {
            if (edge.source === selectedChunkId) neighbors.add(edge.target);
            if (edge.target === selectedChunkId) neighbors.add(edge.source);
        }
        return neighbors;
    }, [selectedChunkId, layoutEdges]);

    const pathResult = useMemo(() => {
        if (!pathStartId || !pathEndId) return null;
        const path = findShortestPath(pathStartId, pathEndId, layoutEdges);
        if (!path) return null;
        const pathNodeIds = new Set(path);
        const pathEdgeIds = new Set<string>();
        for (let i = 0; i < path.length - 1; i++) {
            for (const edge of layoutEdges) {
                if ((edge.source === path[i] && edge.target === path[i + 1]) || (edge.target === path[i] && edge.source === path[i + 1])) {
                    pathEdgeIds.add(edge.id);
                }
            }
        }
        return { pathNodeIds, pathEdgeIds, length: path.length - 1, path };
    }, [pathStartId, pathEndId, layoutEdges]);

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
    }, [nodes]);

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

    // Consolidated node/edge styling effect — single source of truth to prevent cascading setState loops
    useEffect(() => {
        const hasSearch = debouncedSearchQuery.trim().length > 0;

        // --- Compute node styles ---
        let styledNodes: Node[];

        if (pathResult) {
            styledNodes = layoutNodes.map(node => ({
                ...node,
                style: {
                    ...(node.style as Record<string, unknown>),
                    opacity: pathResult.pathNodeIds.has(node.id) ? 1 : 0.1,
                    boxShadow: "none",
                    transition: "opacity 0.2s, box-shadow 0.2s"
                }
            }));
        } else if (hasSearch) {
            const q = debouncedSearchQuery.toLowerCase();
            const matchIds = new Set<string>();
            for (const node of layoutNodes) {
                const label = typeof node.data.label === "string" ? node.data.label : "";
                if (label.toLowerCase().includes(q)) matchIds.add(node.id);
            }
            styledNodes = layoutNodes.map(node => {
                const isMatch = matchIds.has(node.id);
                return {
                    ...node,
                    style: {
                        ...(node.style as Record<string, unknown>),
                        opacity: isMatch ? 1 : 0.15,
                        boxShadow: isMatch ? `0 0 12px 2px ${(node.style as Record<string, string>)?.borderColor ?? "#475569"}` : "none",
                        transition: "opacity 0.2s, box-shadow 0.2s"
                    }
                };
            });
        } else if (focusModeNeighbors) {
            styledNodes = layoutNodes.map(node => ({
                ...node,
                style: {
                    ...(node.style as Record<string, unknown>),
                    opacity: focusModeNeighbors.has(node.id) ? 1 : 0.15,
                    transition: "opacity 0.3s ease"
                }
            }));
        } else if (focusNeighbors) {
            styledNodes = layoutNodes.map(node => ({
                ...node,
                style: {
                    ...(node.style as Record<string, unknown>),
                    opacity: focusNeighbors.has(node.id) ? 1 : 0.12,
                    transition: "opacity 0.2s"
                }
            }));
        } else if (selectedNeighborNodes) {
            styledNodes = layoutNodes.map(node => ({
                ...node,
                style: {
                    ...(node.style as Record<string, unknown>),
                    opacity: selectedNeighborNodes.has(node.id) ? 1 : 0.2,
                    transition: "opacity 0.2s"
                }
            }));
        } else if (chunkTagGroupMap && chunkTagGroupMap.size > 0) {
            styledNodes = layoutNodes.map(node => {
                if (isGroupNodeId(node.id)) return node;
                const inGroup = chunkTagGroupMap.has(node.id);
                return {
                    ...node,
                    style: {
                        ...(node.style as Record<string, unknown>),
                        opacity: inGroup ? 1 : 0.85,
                        transition: "opacity 0.2s"
                    }
                };
            });
        } else {
            styledNodes = layoutNodes;
        }

        // Apply multi-select outline on top
        if (multiSelectedIds.size > 0) {
            styledNodes = styledNodes.map(node => ({
                ...node,
                style: {
                    ...(node.style as Record<string, unknown>),
                    outline: multiSelectedIds.has(node.id) ? "2px solid #f472b6" : "none",
                    outlineOffset: multiSelectedIds.has(node.id) ? "2px" : "0"
                }
            }));
        }

        mergeNodes(styledNodes);

        // --- Compute edge styles ---
        let styledEdges: Edge[];

        if (pathResult) {
            styledEdges = layoutEdges.map(edge => ({
                ...edge,
                style: {
                    ...(edge.style as Record<string, unknown>),
                    opacity: pathResult.pathEdgeIds.has(edge.id) ? 1 : 0.05,
                    strokeWidth: pathResult.pathEdgeIds.has(edge.id) ? 3 : ((edge.style as Record<string, number>)?.strokeWidth ?? 2),
                    transition: "opacity 0.2s"
                }
            }));
        } else if (hasSearch) {
            const q = debouncedSearchQuery.toLowerCase();
            const matchIds = new Set<string>();
            for (const node of layoutNodes) {
                const label = typeof node.data.label === "string" ? node.data.label : "";
                if (label.toLowerCase().includes(q)) matchIds.add(node.id);
            }
            styledEdges = layoutEdges.map(edge => ({
                ...edge,
                style: {
                    ...(edge.style as Record<string, unknown>),
                    opacity: matchIds.has(edge.source) || matchIds.has(edge.target) ? 1 : 0.1
                }
            }));
        } else if (focusModeNeighbors) {
            styledEdges = layoutEdges.map(edge => ({
                ...edge,
                style: {
                    ...(edge.style as Record<string, unknown>),
                    opacity: focusModeNeighbors.has(edge.source) && focusModeNeighbors.has(edge.target) ? 1 : 0.08,
                    transition: "opacity 0.3s ease"
                }
            }));
        } else if (focusNeighbors) {
            styledEdges = layoutEdges.map(edge => ({
                ...edge,
                style: {
                    ...(edge.style as Record<string, unknown>),
                    opacity: focusNeighbors.has(edge.source) && focusNeighbors.has(edge.target) ? 1 : 0.06,
                    transition: "opacity 0.2s"
                }
            }));
        } else if (selectedEdgeIds) {
            styledEdges = layoutEdges.map(edge => ({
                ...edge,
                style: {
                    ...(edge.style as Record<string, unknown>),
                    opacity: selectedEdgeIds.has(edge.id) ? 1 : 0.1,
                    transition: "opacity 0.2s"
                }
            }));
        } else if (chunkTagGroupMap && chunkTagGroupMap.size > 0) {
            styledEdges = layoutEdges.map(edge => {
                const sourceGroups = chunkTagGroupMap.get(edge.source);
                const targetGroups = chunkTagGroupMap.get(edge.target);
                let sameGroup = false;
                if (sourceGroups && targetGroups) {
                    for (const g of sourceGroups) {
                        if (targetGroups.has(g)) { sameGroup = true; break; }
                    }
                }
                return {
                    ...edge,
                    style: {
                        ...(edge.style as Record<string, unknown>),
                        opacity: sameGroup ? 1 : 0.15,
                        transition: "opacity 0.3s ease"
                    }
                };
            });
        } else {
            styledEdges = layoutEdges;
        }

        // Override: restore opacity for selected node's direct connections when tag grouping is active
        if (chunkTagGroupMap && chunkTagGroupMap.size > 0 && selectedChunkId) {
            const selectedDirectEdgeIds = new Set<string>();
            for (const edge of layoutEdges) {
                if (edge.source === selectedChunkId || edge.target === selectedChunkId) {
                    selectedDirectEdgeIds.add(edge.id);
                }
            }
            styledEdges = styledEdges.map(edge => {
                if (selectedDirectEdgeIds.has(edge.id)) {
                    return { ...edge, style: { ...(edge.style as Record<string, unknown>), opacity: 1 } };
                }
                return edge;
            });
        }

        setEdges(styledEdges);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [
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
        selectedChunkId
    ]);


    // Fit view once after first layout positions arrive
    const fitViewRef = useRef(fitView);
    fitViewRef.current = fitView;
    useEffect(() => {
        if (layoutPositions && !initialFitDoneRef.current) {
            initialFitDoneRef.current = true;
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

    // Extracted so the pre-render gate and the normal mount share identical handlers.
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

    // Gate the heavy graph render until the user has committed to a filter.
    // Keeps the data query alive (dialog still shows live preview) but skips
    // ReactFlow mount + layout worker until the user clicks Apply or Show everything.
    const graphGated = filterDialogOpen && !hasAnyFilterParams;
    if (graphGated) {
        const previewData = data?.chunks
            ? {
                  chunks: data.chunks,
                  connections: data.connections ?? [],
                  chunkTags: data.chunkTags ?? []
              }
            : undefined;
        return (
            <div className="relative flex h-[calc(100vh-4rem)] items-center justify-center">
                <div className="max-w-sm text-center text-muted-foreground">
                    <Filter className="mx-auto size-10 opacity-30" />
                    <p className="mt-4 text-sm font-medium text-foreground">Pick a filter first</p>
                    <p className="mt-1 text-xs">
                        The graph renders after you apply a filter or choose "Show everything". This keeps the layout
                        work focused on what you actually want to see.
                    </p>
                </div>
                <GraphFilterDialog
                    open={filterDialogOpen}
                    onOpenChange={setFilterDialogOpen}
                    initial={prefilter}
                    previewData={previewData}
                    onApply={handleFilterApply}
                    onShowEverything={handleShowEverything}
                />
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
                        onNodeClick={(event, node) => {
                            if (isGroupNodeId(node.id)) return;
                            if (event.shiftKey) {
                                dispatch({ type: "TOGGLE_MULTI_SELECT", id: node.id });
                                return;
                            }
                            // Clear multi-select on normal click
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
                            // Toggle focus mode: dims everything beyond 2 hops
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
                                groupDragStartRef.current = { groupId: node.id, startPos: { ...node.position } };
                            }
                        }}
                        onNodeDrag={(_, node) => {
                            if (isGroupNodeId(node.id) && groupDragStartRef.current?.groupId === node.id) {
                                const dx = node.position.x - groupDragStartRef.current.startPos.x;
                                const dy = node.position.y - groupDragStartRef.current.startPos.y;
                                const childIds = groupToChunkIds.get(node.id);
                                if (!childIds) return;
                                setNodes(prev => prev.map(n => {
                                    if (!childIds.includes(n.id)) return n;
                                    const dragged = draggedPositions.get(n.id);
                                    const lp = layoutPositions?.[n.id];
                                    if (!dragged && !lp) return n;
                                    // dragged is top-left, layoutPositions is center — normalize to top-left
                                    let baseX: number, baseY: number;
                                    if (dragged) {
                                        baseX = dragged.x;
                                        baseY = dragged.y;
                                    } else {
                                        const size = measuredNodeSizesRef.current.get(n.id);
                                        baseX = lp!.x - (size?.w ?? 180) / 2;
                                        baseY = lp!.y - (size?.h ?? 36) / 2;
                                    }
                                    return { ...n, position: { x: baseX + dx, y: baseY + dy } };
                                }));
                            }
                        }}
                        onNodeDragStop={(_, node) => {
                            if (isGroupNodeId(node.id) && groupDragStartRef.current?.groupId === node.id) {
                                groupDragStartRef.current = null;
                                const childIds = groupToChunkIds.get(node.id);
                                if (!childIds) return;
                                // Persist the current visual positions of children (already moved during onNodeDrag)
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
                                      chunkTags: data.chunkTags ?? []
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
                            {searchMatchIds.size} of {layoutNodes.filter(n => !isGroupNodeId(n.id)).length}
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

                {/* Top-center: Legend */}
                <GraphLegend activeTypes={activeTypes} activeRelations={activeRelations} onToggleType={toggleType} onToggleRelation={toggleRelation} />

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
                              chunkTags: data.chunkTags ?? []
                          }
                        : undefined
                }
                onApply={handleFilterApply}
                onShowEverything={handleShowEverything}
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
