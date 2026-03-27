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
import { Download, Route, Settings2 } from "lucide-react";
import { useTheme } from "next-themes";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Dialog, DialogPopup, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Popover, PopoverTrigger, PopoverPopup } from "@/components/ui/popover";
import { Sheet, SheetContent } from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { relationColor } from "@/features/chunks/relation-colors";
import { FloatingEdge } from "@/features/graph/floating-edge";
import { runForceLayout } from "@/features/graph/force-layout";
import { GraphDetailPanel } from "@/features/graph/graph-detail-panel";
import { GraphFilters } from "@/features/graph/graph-filters";
import { GraphLegend } from "@/features/graph/graph-legend";
import { GraphMetrics } from "@/features/graph/graph-metrics";
import { GraphNode } from "@/features/graph/graph-node";
import { GraphGroupNode } from "@/features/graph/graph-group-node";
import { GraphWelcome } from "@/features/graph/graph-welcome";
import type { LayoutWorkerInput, LayoutWorkerOutput } from "@/features/graph/layout.worker";
import { type LayoutAlgorithm, hierarchicalLayout, radialLayout } from "@/features/graph/layouts";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { GraphTimeline } from "./graph-timeline";
import { PathPanel } from "./path-panel";
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

function findShortestPath(startId: string, endId: string, edges: Edge[]): string[] | null {
    const adjacency = new Map<string, string[]>();
    for (const edge of edges) {
        if (!adjacency.has(edge.source)) adjacency.set(edge.source, []);
        if (!adjacency.has(edge.target)) adjacency.set(edge.target, []);
        adjacency.get(edge.source)!.push(edge.target);
        adjacency.get(edge.target)!.push(edge.source);
    }
    const visited = new Set<string>([startId]);
    const queue: { nodeId: string; path: string[] }[] = [{ nodeId: startId, path: [startId] }];
    while (queue.length > 0) {
        const { nodeId, path } = queue.shift()!;
        if (nodeId === endId) return path;
        for (const neighbor of adjacency.get(nodeId) ?? []) {
            if (!visited.has(neighbor)) {
                visited.add(neighbor);
                queue.push({ nodeId: neighbor, path: [...path, neighbor] });
            }
        }
    }
    return null;
}

function getMostConnected(nodes: { id: string }[], edges: { source: string; target: string }[]): string | null {
    const counts = new Map<string, number>();
    for (const e of edges) {
        counts.set(e.source, (counts.get(e.source) ?? 0) + 1);
        counts.set(e.target, (counts.get(e.target) ?? 0) + 1);
    }
    let maxId: string | null = null;
    let maxCount = 0;
    for (const [id, count] of counts) {
        if (count > maxCount && nodes.some(n => n.id === id)) {
            maxCount = count;
            maxId = id;
        }
    }
    return maxId;
}

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

    const RELATION_TYPES = [
        "related_to",
        "part_of",
        "depends_on",
        "extends",
        "references",
        "supports",
        "contradicts",
        "alternative_to"
    ] as const;

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
        // Quick-connect: create with "related_to" immediately, offer to change type via toast
        createConnectionMutation.mutate(
            { sourceId: connection.source, targetId: connection.target, relation: "related_to" },
            {
                onSuccess: () => {
                    const src = connection.source!;
                    const tgt = connection.target!;
                    toast.success("Connected", {
                        action: {
                            label: "Change type",
                            onClick: () => dispatch({ type: "SET_PENDING_CONNECTION", connection: { source: src, target: tgt } }),
                        },
                    });
                },
            }
        );
    }, [createConnectionMutation, dispatch]);

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

    // Filter state (empty = show all)
    const allTypes = useMemo(() => [...new Set((data?.chunks ?? []).map(c => c.type))], [data]);
    const allRelations = useMemo(() => [...new Set((data?.connections ?? []).map(c => c.relation))], [data]);

    const debouncedSearchQuery = useDebouncedValue(searchQuery, 150);

    // Measured node sizes for group bounding box calculation (ref + counter to trigger recalc)
    const measuredNodeSizesRef = useRef(new Map<string, { w: number; h: number }>());
    const [measuredSizesVersion, setMeasuredSizesVersion] = useState(0);

    // Read path params from URL search
    const search = useSearch({ strict: false }) as { pathFrom?: string; pathTo?: string };
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
    function toggleTagType(id: string) {
        dispatch({ type: "TOGGLE_TAG_TYPE", id });
    }

    // Tag types for the filter panel
    const tagTypeInfos = useMemo(() => {
        if (!data?.tagTypes) return [];
        const tagCountByType = new Map<string, number>();
        for (const ct of data.chunkTags ?? []) {
            if (ct.tagTypeId) {
                tagCountByType.set(ct.tagTypeId, (tagCountByType.get(ct.tagTypeId) ?? 0) + 1);
            }
        }
        return data.tagTypes.map(tt => ({
            id: tt.id,
            name: tt.name,
            color: tt.color,
            tagCount: tagCountByType.get(tt.id) ?? 0
        }));
    }, [data]);

    // Build tag groups: tagName -> chunkIds (for active tag types only)
    const tagGroups = useMemo(() => {
        if (activeTagTypeIds.size === 0 || !data?.chunkTags) return null;
        const groups = new Map<string, string[]>();
        for (const ct of data.chunkTags) {
            if (!ct.tagTypeId || !activeTagTypeIds.has(ct.tagTypeId)) continue;
            if (!groups.has(ct.tagName)) groups.set(ct.tagName, []);
            groups.get(ct.tagName)!.push(ct.chunkId);
        }
        return groups;
    }, [activeTagTypeIds, data]);

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
        let chunks = data?.chunks ?? [];
        let connections = data?.connections ?? [];

        if (chunks.length === 0) return null;

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
    }, [data, filterTypes, filterRelations, collapsedParents, exploreMode, exploredNodeIds, timelineCutoff]);

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

        if (tagGroups && tagGroups.size > 0 && data?.chunkTags) {
            // Build tag color lookup
            const tagColorMap = new Map<string, string>();
            for (const ct of data.chunkTags) {
                if (ct.tagTypeColor) tagColorMap.set(ct.tagName, ct.tagTypeColor);
            }

            for (const [tagName, chunkIds] of tagGroups) {
                const groupId = `tag-group-${tagName}`;
                tagGroupNodeIds.add(groupId);
                for (const cid of chunkIds) {
                    if (!chunkToGroupId.has(cid)) {
                        chunkToGroupId.set(cid, groupId);
                    }
                }
                const color = tagColorMap.get(tagName) ?? "#8b5cf6";
                rawNodes.unshift({
                    id: groupId,
                    type: "group",
                    data: { label: tagName, color },
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
                const ungroupedId = "tag-group-ungrouped";
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

        // Pre-compute group bounding boxes from child layout positions
        const PADDING = 14;
        const PADDING_TOP = 40;
        const PADDING_BOTTOM = 28;
        const DEFAULT_NODE_W = 180;
        const DEFAULT_NODE_H = 36;
        const groupBounds = new Map<string, { minX: number; minY: number; maxX: number; maxY: number }>();

        const measuredSizes = measuredNodeSizesRef.current;

        if (tagGroupNodeIds.size > 0 && layoutPositions) {
            for (const [chunkId, groupId] of chunkToGroupId) {
                const dragged = draggedPositions.get(chunkId);
                const lp = layoutPositions[chunkId];
                if (!dragged && !lp) continue;
                const size = measuredSizes.get(chunkId);
                const w = size?.w ?? DEFAULT_NODE_W;
                const h = size?.h ?? DEFAULT_NODE_H;
                // dragged positions are top-left, layout positions are centers
                const x = dragged ? dragged.x : lp!.x - w / 2;
                const y = dragged ? dragged.y : lp!.y - h / 2;
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

            // Group nodes: always compute size from bounds, use dragged position if available
            if (tagGroupNodeIds.has(node.id)) {
                const bounds = groupBounds.get(node.id);
                if (bounds) {
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
                return { ...node, position: { x: 0, y: 0 } };
            }

            const dragged = draggedPositions.get(node.id);
            if (dragged) return { ...node, position: dragged };

            const size = measuredSizes.get(node.id);
            const hw = (size?.w ?? DEFAULT_NODE_W) / 2;
            const hh = (size?.h ?? DEFAULT_NODE_H) / 2;
            return { ...node, position: { x: p.x - hw, y: p.y - hh } };
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

    const focusNeighbors = useMemo(() => {
        if (!focusedNodeId) return null;
        const neighbors = new Set<string>([focusedNodeId]);
        for (const edge of layoutEdges) {
            if (edge.source === focusedNodeId) neighbors.add(edge.target);
            if (edge.target === focusedNodeId) neighbors.add(edge.source);
        }
        return neighbors;
    }, [focusedNodeId, layoutEdges]);

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
                if (node.id.startsWith("tag-group-")) return node;
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

    useEffect(() => {
        function handleKeyDown(e: KeyboardEvent) {
            if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

            if (e.key === "?") {
                dispatch({ type: "TOGGLE_HELP" });
                return;
            }

            if (e.key === "Escape") {
                if (multiSelectedIds.size > 0) {
                    dispatch({ type: "CLEAR_MULTI_SELECT" });
                    return;
                }
                if (pathStartId || pathEndId) {
                    dispatch({ type: "CLEAR_PATH" });
                    return;
                }
                if (selectedChunkId) {
                    dispatch({ type: "SET_SELECTED_CHUNK", id: null });
                } else if (focusedNodeId) {
                    dispatch({ type: "SET_FOCUSED_NODE", id: null });
                }
                return;
            }

            if (e.key === "Tab" && selectedChunkId) {
                e.preventDefault();
                const connectedIds: string[] = [];
                for (const edge of layoutEdges) {
                    if (edge.source === selectedChunkId) connectedIds.push(edge.target);
                    if (edge.target === selectedChunkId) connectedIds.push(edge.source);
                }
                if (connectedIds.length === 0) return;
                const currentIdx = connectedIds.indexOf(selectedChunkId);
                const nextIdx = e.shiftKey
                    ? (currentIdx - 1 + connectedIds.length) % connectedIds.length
                    : (currentIdx + 1) % connectedIds.length;
                const nextId = connectedIds[nextIdx]!;
                dispatch({ type: "SELECT_AND_FOCUS_NODE", id: nextId });
            }
        }

        document.addEventListener("keydown", handleKeyDown);
        return () => document.removeEventListener("keydown", handleKeyDown);
    }, [selectedChunkId, focusedNodeId, layoutEdges, pathStartId, pathEndId, multiSelectedIds]);

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
            <div className="relative flex-1 touch-manipulation [&_.react-flow__handle]:invisible [&_.react-flow__node]:transition-[transform] [&_.react-flow__node]:duration-500 [&_.react-flow__node]:ease-out [&_.react-flow__node:hover_.react-flow__handle]:!visible">
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
                            if (node.id.startsWith("tag-group-")) return;
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
                        }}
                        onNodeDoubleClick={(_, node) => {
                            if (selectedChunkId === node.id) {
                                navigate({ to: "/chunks/$chunkId", params: { chunkId: node.id } });
                            } else {
                                dispatch({ type: "TOGGLE_COLLAPSED_PARENT", id: node.id });
                            }
                        }}
                        onNodeMouseEnter={(event, node) => {
                            setHoveredNode({ id: node.id, x: event.clientX, y: event.clientY });
                        }}
                        onNodeDragStart={(_, node) => {
                            if (node.id.startsWith("tag-group-")) {
                                groupDragStartRef.current = { groupId: node.id, startPos: { ...node.position } };
                            }
                        }}
                        onNodeDrag={(_, node) => {
                            if (node.id.startsWith("tag-group-") && groupDragStartRef.current?.groupId === node.id) {
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
                            if (node.id.startsWith("tag-group-") && groupDragStartRef.current?.groupId === node.id) {
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
                                    ["Double-click", "Open chunk / toggle collapse"],
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

                {/* Top-left: Filters */}
                <div className="max-md:hidden">
                    <GraphFilters
                        types={allTypes}
                        relations={allRelations}
                        activeTypes={filterTypes}
                        activeRelations={filterRelations}
                        onToggleType={toggleType}
                        onToggleRelation={toggleRelation}
                        tagTypes={tagTypeInfos}
                        activeTagTypeIds={activeTagTypeIds}
                        onToggleTagType={toggleTagType}
                        edgeAnimated={edgeAnimated}
                        onToggleEdgeAnimated={() => dispatch({ type: "TOGGLE_EDGE_ANIMATED" })}
                        showUngrouped={showUngrouped}
                        onToggleUngrouped={() => dispatch({ type: "TOGGLE_UNGROUPED" })}
                    />
                </div>

                {/* Top-right: Search + Stats */}
                <div className="absolute top-4 right-4 z-10 flex items-center gap-2">
                    <input
                        type="text"
                        value={searchQuery}
                        onChange={e => dispatch({ type: "SET_SEARCH_QUERY", query: e.target.value })}
                        placeholder="Search nodes..."
                        className="bg-background/80 focus:ring-ring w-36 rounded-md border px-2.5 py-1.5 text-xs backdrop-blur-sm focus:ring-2 focus:outline-none"
                    />
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
                    <Popover>
                        <PopoverTrigger className="bg-background/80 text-muted-foreground hover:text-foreground rounded-md border p-1.5 backdrop-blur-sm">
                            <Settings2 className="size-4" />
                        </PopoverTrigger>
                        <PopoverPopup side="bottom" align="end" sideOffset={8} className="w-56">
                            <div className="flex flex-col gap-3">
                                <div>
                                    <label className="text-muted-foreground mb-1 block text-[10px] font-medium tracking-wider uppercase">
                                        Layout
                                    </label>
                                    <select
                                        value={layoutAlgorithm}
                                        onChange={e => dispatch({ type: "SET_LAYOUT_ALGORITHM", algorithm: e.target.value as LayoutAlgorithm })}
                                        className="bg-background w-full rounded-md border px-2 py-1.5 text-xs"
                                    >
                                        <option value="force">Force-directed</option>
                                        <option value="hierarchical">Tree</option>
                                        <option value="radial">Radial</option>
                                    </select>
                                </div>
                                {draggedPositions.size > 0 && (
                                    <button
                                        onClick={() => setDraggedPositions(new Map())}
                                        className="text-muted-foreground hover:text-foreground w-full rounded-md border px-2.5 py-1.5 text-left text-xs"
                                    >
                                        Reset layout
                                    </button>
                                )}
                                <div className="flex flex-col gap-2">
                                    <label className="text-muted-foreground text-[10px] font-medium tracking-wider uppercase">Tools</label>
                                    <button
                                        onClick={() => {
                                            if (!exploreMode) {
                                                dispatch({ type: "SET_EXPLORE_MODE", enabled: true });
                                                dispatch({ type: "SET_EXPLORED_NODE_IDS", ids: selectedChunkId ? new Set([selectedChunkId]) : new Set() });
                                            } else {
                                                dispatch({ type: "SET_EXPLORE_MODE", enabled: false });
                                            }
                                        }}
                                        className={`w-full rounded-md border px-2.5 py-1.5 text-left text-xs ${
                                            exploreMode
                                                ? "bg-primary text-primary-foreground"
                                                : "text-muted-foreground hover:text-foreground"
                                        }`}
                                    >
                                        Explore mode
                                        {exploreMode && <span className="ml-1 opacity-70">({exploredNodeIds.size})</span>}
                                    </button>
                                    {exploreMode && (
                                        <button
                                            onClick={() => dispatch({ type: "SET_EXPLORED_NODE_IDS", ids: new Set() })}
                                            className="text-muted-foreground hover:text-foreground w-full rounded-md border px-2.5 py-1.5 text-left text-xs"
                                        >
                                            Reset explored
                                        </button>
                                    )}
                                    <button
                                        onClick={() => dispatch({ type: "TOGGLE_BUNDLE_EDGES" })}
                                        className={`w-full rounded-md border px-2.5 py-1.5 text-left text-xs ${
                                            bundleEdges
                                                ? "bg-primary text-primary-foreground"
                                                : "text-muted-foreground hover:text-foreground"
                                        }`}
                                    >
                                        Bundle edges
                                    </button>
                                    <button
                                        onClick={() => dispatch({ type: "TOGGLE_USE_MAIN_THREAD" })}
                                        className={`w-full rounded-md border px-2.5 py-1.5 text-left text-xs ${
                                            useMainThread
                                                ? "bg-primary text-primary-foreground"
                                                : "text-muted-foreground hover:text-foreground"
                                        }`}
                                    >
                                        Main-thread layout
                                    </button>
                                </div>
                                <div className="flex flex-col gap-2">
                                    <label className="text-muted-foreground text-[10px] font-medium tracking-wider uppercase">Views</label>
                                    <button
                                        onClick={() => dispatch({ type: "SET_SHOW_SAVE_DIALOG", show: true })}
                                        className="text-muted-foreground hover:text-foreground w-full rounded-md border px-2.5 py-1.5 text-left text-xs"
                                    >
                                        Save current view...
                                    </button>
                                    {savedViews.map(view => (
                                        <div key={view.name} className="flex items-center justify-between gap-1">
                                            <button
                                                className="text-muted-foreground hover:text-foreground flex-1 truncate rounded-md border px-2.5 py-1.5 text-left text-xs"
                                                onClick={() => dispatch({
                                                    type: "RESTORE_VIEW",
                                                    filterTypes: view.filterTypes,
                                                    filterRelations: view.filterRelations,
                                                    collapsedParents: view.collapsedParents,
                                                    layoutAlgorithm: view.layoutAlgorithm as LayoutAlgorithm,
                                                    focusNodeId: view.focusNodeId,
                                                })}
                                            >
                                                {view.name}
                                            </button>
                                            <button
                                                className="text-muted-foreground hover:text-destructive shrink-0 rounded p-1"
                                                onClick={() => deleteSavedView(view.name)}
                                            >
                                                <span className="text-[10px]">✕</span>
                                            </button>
                                        </div>
                                    ))}
                                </div>
                                <div className="border-t pt-3">
                                    <button
                                        onClick={handleExportImage}
                                        className="text-muted-foreground hover:text-foreground flex w-full items-center gap-2 rounded-md border px-2.5 py-1.5 text-xs"
                                    >
                                        <Download className="size-3.5" />
                                        Export as PNG
                                    </button>
                                </div>
                            </div>
                        </PopoverPopup>
                    </Popover>
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
                {/* Edge creation dialog */}
                {/* Change type dialog (opened from quick-connect toast) */}
                <Dialog
                    open={!!pendingConnection}
                    onOpenChange={open => {
                        if (!open) dispatch({ type: "SET_PENDING_CONNECTION", connection: null });
                    }}
                >
                    <DialogPopup className="max-w-sm">
                        <DialogHeader>
                            <DialogTitle>Change Connection Type</DialogTitle>
                            <p className="text-muted-foreground text-sm">
                                <span className="text-foreground font-medium">
                                    {pendingConnection ? (chunkMap.get(pendingConnection.source)?.title ?? pendingConnection.source) : ""}
                                </span>
                                {" \u2192 "}
                                <span className="text-foreground font-medium">
                                    {pendingConnection ? (chunkMap.get(pendingConnection.target)?.title ?? pendingConnection.target) : ""}
                                </span>
                            </p>
                        </DialogHeader>
                        <div className="grid grid-cols-2 gap-2 px-6 pb-6">
                            {RELATION_TYPES.map(rel => (
                                <button
                                    key={rel}
                                    disabled={createConnectionMutation.isPending}
                                    onClick={() => {
                                        if (!pendingConnection) return;
                                        createConnectionMutation.mutate({
                                            sourceId: pendingConnection.source,
                                            targetId: pendingConnection.target,
                                            relation: rel
                                        });
                                    }}
                                    className="hover:bg-muted rounded-md border-2 px-3 py-2 text-sm font-medium transition-colors disabled:opacity-50"
                                    style={{ borderColor: relationColor(rel) }}
                                >
                                    {rel.replace(/_/g, " ")}
                                </button>
                            ))}
                        </div>
                    </DialogPopup>
                </Dialog>

                {/* Bottom-center: Timeline */}
                <GraphTimeline chunks={data?.chunks ?? []} onCutoffChange={handleTimelineCutoff} />

                {/* Bottom-left: Metrics */}
                <GraphMetrics
                    nodes={nodes}
                    edges={edges}
                    onNodeClick={id => dispatch({ type: "SELECT_AND_FOCUS_NODE", id })}
                />

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
                {showSaveDialog && (
                    <div
                        className="bg-background/50 absolute inset-0 z-30 flex items-center justify-center backdrop-blur-sm"
                        onClick={() => dispatch({ type: "SET_SHOW_SAVE_DIALOG", show: false })}
                    >
                        <div className="bg-background rounded-lg border p-4 shadow-lg" onClick={e => e.stopPropagation()}>
                            <h3 className="mb-2 text-sm font-semibold">Save View</h3>
                            <input
                                value={viewName}
                                onChange={e => dispatch({ type: "SET_VIEW_NAME", name: e.target.value })}
                                placeholder="View name"
                                className="w-full rounded-md border px-3 py-2 text-sm"
                                autoFocus
                                onKeyDown={e => {
                                    if (e.key === "Enter" && viewName.trim()) {
                                        saveView({
                                            name: viewName.trim(),
                                            filterTypes: [...filterTypes],
                                            filterRelations: [...filterRelations],
                                            collapsedParents: [...collapsedParents],
                                            layoutAlgorithm,
                                            focusNodeId: focusedNodeId ?? undefined
                                        });
                                        dispatch({ type: "SET_SHOW_SAVE_DIALOG", show: false });
                                        dispatch({ type: "SET_VIEW_NAME", name: "" });
                                    }
                                }}
                            />
                            <div className="mt-3 flex gap-2">
                                <button
                                    onClick={() => {
                                        if (!viewName.trim()) return;
                                        saveView({
                                            name: viewName.trim(),
                                            filterTypes: [...filterTypes],
                                            filterRelations: [...filterRelations],
                                            collapsedParents: [...collapsedParents],
                                            layoutAlgorithm,
                                            focusNodeId: focusedNodeId ?? undefined
                                        });
                                        dispatch({ type: "SET_SHOW_SAVE_DIALOG", show: false });
                                        dispatch({ type: "SET_VIEW_NAME", name: "" });
                                    }}
                                    className="bg-primary text-primary-foreground flex-1 rounded-md px-3 py-1.5 text-xs"
                                >
                                    Save
                                </button>
                                <button onClick={() => dispatch({ type: "SET_SHOW_SAVE_DIALOG", show: false })} className="rounded-md border px-3 py-1.5 text-xs">
                                    Cancel
                                </button>
                            </div>
                        </div>
                    </div>
                )}
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
