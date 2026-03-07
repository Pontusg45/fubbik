import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import "@xyflow/react/dist/style.css";
import { Background, BackgroundVariant, ConnectionMode, Controls, MiniMap, ReactFlow, ReactFlowProvider, useEdgesState, useNodesState, useReactFlow, type Connection, type Edge, type Node } from "@xyflow/react";
import { toPng } from "html-to-image";
import { Download } from "lucide-react";
import { useTheme } from "next-themes";
import { useCallback, createContext, useEffect, useMemo, useRef, useState } from "react";

import { Dialog, DialogPopup, DialogHeader, DialogTitle } from "@/components/ui/dialog";

import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { relationColor } from "@/features/chunks/relation-colors";
import { FloatingEdge } from "@/features/graph/floating-edge";
import { Spinner } from "@/components/ui/spinner";
import type { LayoutWorkerInput, LayoutWorkerOutput } from "@/features/graph/layout.worker";
import { Sheet, SheetContent } from "@/components/ui/sheet";
import { GraphDetailPanel } from "@/features/graph/graph-detail-panel";
import { GraphFilters } from "@/features/graph/graph-filters";
import { GraphLegend } from "@/features/graph/graph-legend";
import { GraphNode } from "@/features/graph/graph-node";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const ZoomContext = createContext(1);

const MAIN_NODE_ID = "__main__";

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

function GraphViewInner() {
    const { resolvedTheme } = useTheme();
    const isDark = resolvedTheme !== "light";
    const TYPE_COLORS = isDark ? TYPE_COLORS_DARK : TYPE_COLORS_LIGHT;

    const [zoomLevel, setZoomLevel] = useState(1);
    const [selectedChunkId, setSelectedChunkId] = useState<string | null>(null);
    const [panelWidth, setPanelWidth] = useState(380);
    const { setCenter, getZoom } = useReactFlow();
    const navigate = useNavigate();
    const queryClient = useQueryClient();

    // Edge creation state
    const [pendingConnection, setPendingConnection] = useState<{ source: string; target: string } | null>(null);

    const RELATION_TYPES = ["related_to", "part_of", "depends_on", "extends", "references", "supports", "contradicts", "alternative_to"] as const;

    const createConnectionMutation = useMutation({
        mutationFn: async ({ sourceId, targetId, relation }: { sourceId: string; targetId: string; relation: string }) => {
            const { error } = await api.api.connections.post({ sourceId, targetId, relation });
            if (error) throw new Error("Failed to create connection");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["graph"] });
            setPendingConnection(null);
        }
    });

    const onConnect = useCallback((connection: Connection) => {
        if (!connection.source || !connection.target) return;
        if (connection.source === MAIN_NODE_ID || connection.target === MAIN_NODE_ID) return;
        setPendingConnection({ source: connection.source, target: connection.target });
    }, []);

    const [isMobile, setIsMobile] = useState(false);

    useEffect(() => {
        const mql = window.matchMedia("(max-width: 768px)");
        setIsMobile(mql.matches);
        function handler(e: MediaQueryListEvent) { setIsMobile(e.matches); }
        mql.addEventListener("change", handler);
        return () => mql.removeEventListener("change", handler);
    }, []);

    const { data, isLoading } = useQuery({
        queryKey: ["graph"],
        queryFn: async () => {
            return unwrapEden(await api.api.graph.get());
        }
    });

    // Filter state (empty = show all)
    const allTypes = useMemo(() => [...new Set((data?.chunks ?? []).map(c => c.type))], [data]);
    const allRelations = useMemo(() => [...new Set((data?.connections ?? []).map(c => c.relation))], [data]);
    const [filterTypes, setFilterTypes] = useState<Set<string>>(new Set());
    const [filterRelations, setFilterRelations] = useState<Set<string>>(new Set());

    // Search state
    const [searchQuery, setSearchQuery] = useState("");
    const debouncedSearchQuery = useDebouncedValue(searchQuery, 150);

    // Focus mode state
    const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null);

    // Path highlighting state
    const [pathStartId, setPathStartId] = useState<string | null>(null);
    const [pathEndId, setPathEndId] = useState<string | null>(null);

    // Collapsible clusters
    const [collapsedParents, setCollapsedParents] = useState<Set<string>>(new Set());

    // Explore mode
    const [exploreMode, setExploreMode] = useState(false);
    const [exploredNodeIds, setExploredNodeIds] = useState<Set<string>>(new Set());

    // Dragged node positions (persist across layout changes)
    const [draggedPositions, setDraggedPositions] = useState<Map<string, { x: number; y: number }>>(new Map());

    // Active types/relations for legend
    const activeTypes = useMemo(() => new Set((data?.chunks ?? []).map(c => c.type)), [data]);
    const activeRelations = useMemo(() => new Set((data?.connections ?? []).map(c => c.relation)), [data]);

    // Toggle helpers
    function toggleType(t: string) {
        setFilterTypes(prev => {
            const next = new Set(prev);
            if (next.has(t)) next.delete(t);
            else next.add(t);
            return next;
        });
    }
    function toggleRelation(r: string) {
        setFilterRelations(prev => {
            const next = new Set(prev);
            if (next.has(r)) next.delete(r);
            else next.add(r);
            return next;
        });
    }

    // --- Web Worker for force-directed layout ---
    const workerRef = useRef<Worker | null>(null);
    const requestIdRef = useRef<number>(0);
    const [layoutPositions, setLayoutPositions] = useState<Record<string, { x: number; y: number }> | null>(null);
    const [isLayouting, setIsLayouting] = useState(false);

    // Create / teardown the worker
    useEffect(() => {
        const worker = new Worker(
            new URL("./layout.worker.ts", import.meta.url),
            { type: "module" }
        );
        worker.onmessage = (e: MessageEvent<LayoutWorkerOutput>) => {
            if (e.data.requestId !== requestIdRef.current) return;
            setLayoutPositions(e.data.positions);
            setIsLayouting(false);
        };
        worker.onerror = (err) => {
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

        return { chunks, connections, parentChildren, childIds, hiddenIds };
    }, [data, filterTypes, filterRelations, collapsedParents, exploreMode, exploredNodeIds]);

    // Post to worker when simulation inputs change
    useEffect(() => {
        if (!filteredGraph || !workerRef.current) return;

        const { chunks, connections, hiddenIds } = filteredGraph;

        // Build worker input: nodes need id and type for clustering
        const workerNodes: LayoutWorkerInput["nodes"] = [
            { id: MAIN_NODE_ID, type: "__main__" },
            ...chunks
                .filter(c => !hiddenIds.has(c.id))
                .map(c => ({ id: c.id, type: c.type }))
        ];

        // Include orphan-to-main edges so the worker can account for spring forces on orphans
        const connectedIds = new Set<string>();
        for (const conn of connections) {
            connectedIds.add(conn.sourceId);
            connectedIds.add(conn.targetId);
        }

        const workerEdges: LayoutWorkerInput["edges"] = connections.map(conn => ({
            source: conn.sourceId,
            target: conn.targetId,
            relation: conn.relation
        }));

        // Add orphan edges
        for (const c of chunks) {
            if (!hiddenIds.has(c.id) && !connectedIds.has(c.id)) {
                workerEdges.push({ source: MAIN_NODE_ID, target: c.id, relation: "" });
            }
        }

        setIsLayouting(true);
        requestIdRef.current += 1;
        workerRef.current.postMessage({ requestId: requestIdRef.current, nodes: workerNodes, edges: workerEdges } satisfies LayoutWorkerInput);
    }, [filteredGraph]);

    // Build layoutNodes and layoutEdges from positions (cheap: styling + edge creation only)
    const { layoutNodes, layoutEdges } = useMemo(() => {
        if (!filteredGraph) return { layoutNodes: [] as Node[], layoutEdges: [] as Edge[] };

        const { chunks, connections, parentChildren, childIds, hiddenIds } = filteredGraph;

        // Connection counts for node sizing
        const connectionCounts = new Map<string, number>();
        for (const conn of connections) {
            connectionCounts.set(conn.sourceId, (connectionCounts.get(conn.sourceId) ?? 0) + 1);
            connectionCounts.set(conn.targetId, (connectionCounts.get(conn.targetId) ?? 0) + 1);
        }

        const rawNodes: Node[] = [
            {
                id: MAIN_NODE_ID,
                data: { label: "Knowledge Base" },
                position: { x: 0, y: 0 },
                style: {
                    cursor: "default",
                    background: isDark ? "#0f172a" : "#ffffff",
                    borderColor: isDark ? "#e2e8f0" : "#1e293b",
                    borderWidth: 2,
                    borderRadius: 12,
                    color: isDark ? "#f8fafc" : "#0f172a",
                    fontSize: 14,
                    fontWeight: 600,
                    padding: "10px 18px"
                }
            },
            ...chunks
                .filter(c => !hiddenIds.has(c.id))
                .map(c => {
                    const typeColor = TYPE_COLORS[c.type] ?? TYPE_COLORS.note;
                    const count = connectionCounts.get(c.id) ?? 0;
                    const scale = Math.min(1 + count * 0.08, 1.8);
                    const isParent = parentChildren.has(c.id);
                    const isChild = childIds.has(c.id);
                    const childCount = collapsedParents.has(c.id) ? parentChildren.get(c.id)?.size ?? 0 : 0;
                    const label = childCount > 0 ? `${c.title} (${childCount})` : c.title;
                    const baseFontSize = isParent ? 13 : 12;
                    const baseVPad = isParent ? 10 : 8;
                    const baseHPad = isParent ? 16 : 14;
                    return {
                        id: c.id,
                        type: "chunk",
                        data: { label, type: c.type, connectionCount: connectionCounts.get(c.id) ?? 0, scale, tags: c.tags as string[] },
                        position: { x: 0, y: 0 },
                        style: {
                            cursor: "pointer",
                            background: typeColor!.bg,
                            borderColor: typeColor!.border,
                            borderWidth: isParent ? 2 : collapsedParents.has(c.id) ? 2.5 : 1.5,
                            borderRadius: isParent ? 12 : 10,
                            color: isDark ? (isChild ? "#cbd5e1" : "#e2e8f0") : (isChild ? "#475569" : "#1e293b"),
                            fontSize: Math.round(baseFontSize * Math.min(scale, 1.3)),
                            fontWeight: isParent ? 600 : 500,
                            padding: `${Math.round(baseVPad * scale)}px ${Math.round(baseHPad * scale)}px`,
                            minWidth: `${Math.round((isParent ? 200 : 180) * scale)}px`,
                            letterSpacing: "0.01em",
                            boxShadow: count >= 5 ? `0 0 ${Math.round(count * 2)}px 1px ${typeColor!.border}40` : undefined
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
                type: "floating",
                data: { relation: conn.relation, directed: true },
                animated: true,
                style: { stroke: color, strokeWidth: 2, transition: "opacity 0.3s ease" }
            };
        });

        // Link orphan nodes to the main node
        const connectedIds = new Set<string>();
        for (const conn of connections) {
            connectedIds.add(conn.sourceId);
            connectedIds.add(conn.targetId);
        }
        for (const c of chunks) {
            if (!hiddenIds.has(c.id) && !connectedIds.has(c.id)) {
                rawEdges.push({
                    id: `main-${c.id}`,
                    source: MAIN_NODE_ID,
                    target: c.id,
                    type: "floating",
                    data: { directed: false },
                    animated: false,
                    style: { stroke: isDark ? "#334155" : "#cbd5e1", strokeWidth: 1, strokeDasharray: "4 4", transition: "opacity 0.3s ease" }
                });
            }
        }

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

        // Apply positions from worker (or fallback to origin)
        const layoutNodes = rawNodes.map(node => {
            const dragged = draggedPositions.get(node.id);
            if (dragged) return { ...node, position: dragged };
            const p = layoutPositions?.[node.id] ?? { x: 0, y: 0 };
            return { ...node, position: { x: p.x - 100, y: p.y - 25 } };
        });

        return { layoutNodes, layoutEdges: rawEdges };
    }, [filteredGraph, layoutPositions, draggedPositions, isDark, TYPE_COLORS, collapsedParents]);

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
                if ((edge.source === path[i] && edge.target === path[i + 1]) ||
                    (edge.target === path[i] && edge.source === path[i + 1])) {
                    pathEdgeIds.add(edge.id);
                }
            }
        }
        return { pathNodeIds, pathEdgeIds, length: path.length - 1 };
    }, [pathStartId, pathEndId, layoutEdges]);

    const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
    const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);

    /** Preserve React DOM identity so CSS transform transitions work. */
    function mergeNodes(newNodes: Node[]) {
        setNodes(prev => {
            if (prev.length === 0) return newNodes;
            const prevMap = new Map(prev.map(n => [n.id, n]));
            return newNodes.map(node => {
                const existing = prevMap.get(node.id);
                if (existing) {
                    return { ...existing, position: node.position, style: node.style, data: node.data };
                }
                return node;
            });
        });
    }
    const [hoveredNode, setHoveredNode] = useState<{ id: string; x: number; y: number } | null>(null);

    const chunkMap = useMemo(() => {
        const map = new Map<string, { title: string; type: string; tags: string[]; summary: string | null }>();
        for (const c of data?.chunks ?? []) {
            map.set(c.id, { title: c.title, type: c.type, tags: c.tags as string[], summary: c.summary });
        }
        return map;
    }, [data]);

    // Apply search highlighting
    useEffect(() => {
        if (!debouncedSearchQuery.trim()) {
            mergeNodes(layoutNodes);
            setEdges(layoutEdges);
            return;
        }
        const q = debouncedSearchQuery.toLowerCase();
        const matchIds = new Set<string>();
        for (const node of layoutNodes) {
            if (node.id === MAIN_NODE_ID) continue;
            const label = typeof node.data.label === "string" ? node.data.label : "";
            if (label.toLowerCase().includes(q)) matchIds.add(node.id);
        }
        mergeNodes(
            layoutNodes.map(node => {
                if (node.id === MAIN_NODE_ID) return node;
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
            })
        );
        setEdges(
            layoutEdges.map(edge => ({
                ...edge,
                style: {
                    ...(edge.style as Record<string, unknown>),
                    opacity: matchIds.has(edge.source) || matchIds.has(edge.target) ? 1 : 0.1
                }
            }))
        );
    }, [debouncedSearchQuery, layoutNodes, layoutEdges, setNodes, setEdges]);

    // Apply focus dimming
    useEffect(() => {
        if (!focusNeighbors || debouncedSearchQuery.trim()) return;
        mergeNodes(
            layoutNodes.map(node => ({
                ...node,
                style: {
                    ...(node.style as Record<string, unknown>),
                    opacity: focusNeighbors.has(node.id) ? 1 : 0.12,
                    transition: "opacity 0.2s"
                }
            }))
        );
        setEdges(
            layoutEdges.map(edge => ({
                ...edge,
                style: {
                    ...(edge.style as Record<string, unknown>),
                    opacity: focusNeighbors.has(edge.source) && focusNeighbors.has(edge.target) ? 1 : 0.06,
                    transition: "opacity 0.2s"
                }
            }))
        );
    }, [focusNeighbors, layoutNodes, layoutEdges, setNodes, setEdges, debouncedSearchQuery]);

    // Sync layout when search is empty and no focus
    useEffect(() => {
        if (!debouncedSearchQuery.trim() && !focusedNodeId) {
            if (selectedNeighborNodes) {
                mergeNodes(layoutNodes.map(node => ({
                    ...node,
                    style: {
                        ...(node.style as Record<string, unknown>),
                        opacity: selectedNeighborNodes.has(node.id) ? 1 : 0.2,
                        transition: "opacity 0.2s"
                    }
                })));
            } else {
                mergeNodes(layoutNodes);
            }
        }
    }, [layoutNodes, setNodes, debouncedSearchQuery, focusedNodeId, selectedNeighborNodes]);

    useEffect(() => {
        if (!debouncedSearchQuery.trim() && !focusedNodeId) {
            if (selectedEdgeIds) {
                setEdges(layoutEdges.map(edge => ({
                    ...edge,
                    style: {
                        ...(edge.style as Record<string, unknown>),
                        opacity: selectedEdgeIds.has(edge.id) ? 1 : 0.1,
                        transition: "opacity 0.2s"
                    }
                })));
            } else {
                setEdges(layoutEdges);
            }
        }
    }, [layoutEdges, setEdges, debouncedSearchQuery, focusedNodeId, selectedEdgeIds]);

    // Apply path highlighting
    useEffect(() => {
        if (!pathResult) return;
        mergeNodes(
            layoutNodes.map(node => ({
                ...node,
                style: {
                    ...(node.style as Record<string, unknown>),
                    opacity: pathResult.pathNodeIds.has(node.id) ? 1 : 0.1,
                    boxShadow: pathResult.pathNodeIds.has(node.id) ? "0 0 16px 4px #f472b6" : "none",
                    transition: "opacity 0.2s, box-shadow 0.2s"
                }
            }))
        );
        setEdges(
            layoutEdges.map(edge => ({
                ...edge,
                style: {
                    ...(edge.style as Record<string, unknown>),
                    opacity: pathResult.pathEdgeIds.has(edge.id) ? 1 : 0.05,
                    strokeWidth: pathResult.pathEdgeIds.has(edge.id) ? 3 : (edge.style as Record<string, number>)?.strokeWidth ?? 2,
                    transition: "opacity 0.2s"
                }
            }))
        );
    }, [pathResult, layoutNodes, layoutEdges, setEdges]);

    // eslint-disable-next-line react-hooks/exhaustive-deps
    useEffect(() => {
        if (!selectedChunkId) return;
        const node = nodes.find(n => n.id === selectedChunkId);
        if (!node) return;
        const x = node.position.x + ((node.measured?.width ?? 180) / 2);
        const y = node.position.y + ((node.measured?.height ?? 40) / 2);
        setCenter(x, y, { duration: 400, zoom: getZoom() });
    }, [selectedChunkId]);

    useEffect(() => {
        function handleKeyDown(e: KeyboardEvent) {
            if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

            if (e.key === "Escape") {
                if (pathStartId || pathEndId) {
                    setPathStartId(null);
                    setPathEndId(null);
                    return;
                }
                if (selectedChunkId) {
                    setSelectedChunkId(null);
                } else if (focusedNodeId) {
                    setFocusedNodeId(null);
                }
                return;
            }

            if (e.key === "Tab" && selectedChunkId) {
                e.preventDefault();
                const connectedIds: string[] = [];
                for (const edge of layoutEdges) {
                    if (edge.source === selectedChunkId && edge.target !== MAIN_NODE_ID) connectedIds.push(edge.target);
                    if (edge.target === selectedChunkId && edge.source !== MAIN_NODE_ID) connectedIds.push(edge.source);
                }
                if (connectedIds.length === 0) return;
                const currentIdx = connectedIds.indexOf(selectedChunkId);
                const nextIdx = e.shiftKey
                    ? (currentIdx - 1 + connectedIds.length) % connectedIds.length
                    : (currentIdx + 1) % connectedIds.length;
                const nextId = connectedIds[nextIdx]!;
                setSelectedChunkId(nextId);
                setFocusedNodeId(nextId);
            }
        }

        document.addEventListener("keydown", handleKeyDown);
        return () => document.removeEventListener("keydown", handleKeyDown);
    }, [selectedChunkId, focusedNodeId, layoutEdges, pathStartId, pathEndId]);

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
                                onClose={() => setSelectedChunkId(null)}
                                onNavigateToChunk={(id) => {
                                    setSelectedChunkId(id);
                                    setFocusedNodeId(id);
                                }}
                            />
                        </div>
                    )}
                    {selectedChunkId && (
                        <div
                            className="absolute top-0 right-0 h-full w-1 cursor-col-resize hover:bg-primary/30 active:bg-primary/50"
                            onMouseDown={(e) => {
                                e.preventDefault();
                                const startX = e.clientX;
                                const startWidth = panelWidth;
                                function onMouseMove(ev: MouseEvent) {
                                    const newWidth = Math.max(280, Math.min(600, startWidth + ev.clientX - startX));
                                    setPanelWidth(newWidth);
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
                <Sheet open={!!selectedChunkId} onOpenChange={(open) => { if (!open) setSelectedChunkId(null); }}>
                    <SheetContent side="bottom" showCloseButton={false} className="h-[70vh] overflow-y-auto p-0">
                        {selectedChunkId && (
                            <GraphDetailPanel
                                chunkId={selectedChunkId}
                                onClose={() => setSelectedChunkId(null)}
                                onNavigateToChunk={(id) => {
                                    setSelectedChunkId(id);
                                    setFocusedNodeId(id);
                                }}
                            />
                        )}
                    </SheetContent>
                </Sheet>
            )}
            <div className="relative flex-1 [&_.react-flow__handle]:invisible [&_.react-flow__node:hover_.react-flow__handle]:!visible [&_.react-flow__node]:transition-[transform] [&_.react-flow__node]:duration-500 [&_.react-flow__node]:ease-out">
            {showLayoutSpinner && (
                <div className="absolute inset-0 z-50 flex items-center justify-center bg-background/60 backdrop-blur-sm">
                    <div className="flex items-center gap-2 text-muted-foreground">
                        <Spinner className="size-5" />
                        <span className="text-sm">Computing layout...</span>
                    </div>
                </div>
            )}
            <ZoomContext.Provider value={zoomLevel}>
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
                    if (node.id === MAIN_NODE_ID) return;
                    if (event.altKey) {
                        if (!pathStartId) {
                            setPathStartId(node.id);
                            setPathEndId(null);
                        } else if (!pathEndId) {
                            setPathEndId(node.id);
                        } else {
                            setPathStartId(node.id);
                            setPathEndId(null);
                        }
                        return;
                    }
                    if (exploreMode) {
                        setExploredNodeIds(prev => {
                            const next = new Set(prev);
                            next.add(node.id);
                            for (const edge of layoutEdges) {
                                if (edge.source === node.id && edge.target !== MAIN_NODE_ID) next.add(edge.target);
                                if (edge.target === node.id && edge.source !== MAIN_NODE_ID) next.add(edge.source);
                            }
                            return next;
                        });
                        setSelectedChunkId(node.id);
                        setFocusedNodeId(node.id);
                        return;
                    }
                    setPathStartId(null);
                    setPathEndId(null);
                    setSelectedChunkId(node.id);
                    setFocusedNodeId(node.id);
                }}
                onPaneClick={() => {
                    setFocusedNodeId(null);
                    setSelectedChunkId(null);
                }}
                onNodeDoubleClick={(_, node) => {
                    if (node.id === MAIN_NODE_ID) return;
                    if (selectedChunkId === node.id) {
                        navigate({ to: "/chunks/$chunkId", params: { chunkId: node.id } });
                    } else {
                        setCollapsedParents(prev => {
                            const next = new Set(prev);
                            if (next.has(node.id)) next.delete(node.id);
                            else next.add(node.id);
                            return next;
                        });
                    }
                }}
                onNodeMouseEnter={(event, node) => {
                    if (node.id === MAIN_NODE_ID) return;
                    setHoveredNode({ id: node.id, x: event.clientX, y: event.clientY });
                }}
                onNodeDragStop={(_, node) => {
                    setDraggedPositions(prev => {
                        const next = new Map(prev);
                        next.set(node.id, node.position);
                        return next;
                    });
                }}
                onNodeMouseLeave={() => setHoveredNode(null)}
                onMoveEnd={(_, viewport) => setZoomLevel(viewport.zoom)}
                onEdgeMouseEnter={(_, edge) => {
                    if (zoomLevel < 0.4) return;
                    setEdges(es =>
                        es.map(e =>
                            e.id === edge.id
                                ? {
                                      ...e,
                                      label: (e.data as { relation?: string })?.relation,
                                      labelStyle: { fill: (e.style as Record<string, string>)?.stroke, fontSize: 10, fontWeight: 500 },
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
                fitView
                colorMode={isDark ? "dark" : "light"}
            >
                <Background variant={BackgroundVariant.Dots} gap={20} size={1} color={isDark ? "rgba(148,163,184,0.15)" : "rgba(100,116,139,0.2)"} />
                <Controls />
                <MiniMap
                    nodeColor={node => {
                        if (node.id === MAIN_NODE_ID) return isDark ? "#e2e8f0" : "#1e293b";
                        if (node.id === selectedChunkId) return "#f472b6";
                        const style = node.style as Record<string, string> | undefined;
                        return style?.borderColor ?? "#475569";
                    }}
                    maskColor={isDark ? "rgba(0, 0, 0, 0.7)" : "rgba(255, 255, 255, 0.7)"}
                    pannable
                    zoomable
                />
            </ReactFlow>
            </ZoomContext.Provider>

            {/* Top-left: Filters */}
            <div className="max-md:hidden">
                <GraphFilters
                    types={allTypes}
                    relations={allRelations}
                    activeTypes={filterTypes}
                    activeRelations={filterRelations}
                    onToggleType={toggleType}
                    onToggleRelation={toggleRelation}
                />
            </div>

            {/* Top-right: Search + Stats */}
            <div className="absolute top-4 right-4 z-10 flex items-center gap-2">
                <input
                    type="text"
                    value={searchQuery}
                    onChange={e => setSearchQuery(e.target.value)}
                    placeholder="Search nodes..."
                    className="bg-background/80 focus:ring-ring w-36 rounded-md border px-2.5 py-1.5 text-xs backdrop-blur-sm focus:ring-2 focus:outline-none"
                />
                {draggedPositions.size > 0 && (
                    <button
                        onClick={() => setDraggedPositions(new Map())}
                        className="text-muted-foreground hover:text-foreground rounded-md border bg-background/80 px-2.5 py-1.5 text-xs backdrop-blur-sm"
                    >
                        Reset layout
                    </button>
                )}
                <button
                    onClick={() => {
                        if (!exploreMode) {
                            setExploreMode(true);
                            setExploredNodeIds(selectedChunkId ? new Set([selectedChunkId]) : new Set());
                        } else {
                            setExploreMode(false);
                            setExploredNodeIds(new Set());
                        }
                    }}
                    className={`rounded-md border px-2.5 py-1.5 text-xs backdrop-blur-sm ${
                        exploreMode ? "bg-primary text-primary-foreground" : "bg-background/80 text-muted-foreground hover:text-foreground"
                    }`}
                    title="Explore mode: click to expand neighbors"
                >
                    Explore
                </button>
                {exploreMode && (
                    <span className="rounded-md border bg-background/80 px-2.5 py-1.5 text-xs text-muted-foreground backdrop-blur-sm">
                        {exploredNodeIds.size} explored
                        <button onClick={() => setExploredNodeIds(new Set())} className="ml-1.5 underline hover:text-foreground">reset</button>
                    </span>
                )}
                <button
                    onClick={handleExportImage}
                    className="text-muted-foreground hover:text-foreground rounded-md border bg-background/80 px-2.5 py-1.5 text-xs backdrop-blur-sm"
                    title="Export as PNG"
                >
                    <Download className="size-3.5" />
                </button>
                <span className="text-muted-foreground rounded-lg border bg-background/80 px-3 py-1.5 text-xs backdrop-blur-sm">
                    {nodes.length - 1} · {edges.length}
                </span>
            </div>

            {/* Top-center: Legend */}
            <GraphLegend activeTypes={activeTypes} activeRelations={activeRelations} />

            {/* Tooltip */}
            {hoveredNode && chunkMap.get(hoveredNode.id) && (() => {
                const info = chunkMap.get(hoveredNode.id)!;
                return (
                    <div
                        className="pointer-events-none fixed z-50 max-w-xs rounded-lg border bg-popover p-3 text-popover-foreground shadow-lg"
                        style={{ left: hoveredNode.x + 12, top: hoveredNode.y + 12 }}
                    >
                        <p className="text-sm font-semibold">{info.title}</p>
                        <p className="text-muted-foreground mt-0.5 text-xs">{info.type}</p>
                        {info.summary && <p className="mt-1.5 text-xs">{info.summary}</p>}
                        {info.tags.length > 0 && (
                            <div className="mt-1.5 flex flex-wrap gap-1">
                                {info.tags.slice(0, 5).map(tag => (
                                    <span key={tag} className="rounded bg-muted px-1.5 py-0.5 text-[10px]">{tag}</span>
                                ))}
                            </div>
                        )}
                    </div>
                );
            })()}
            {/* Edge creation dialog */}
            <Dialog open={!!pendingConnection} onOpenChange={(open) => { if (!open) setPendingConnection(null); }}>
                <DialogPopup className="max-w-sm">
                    <DialogHeader>
                        <DialogTitle>Create Connection</DialogTitle>
                        <p className="text-sm text-muted-foreground">
                            <span className="font-medium text-foreground">{pendingConnection ? chunkMap.get(pendingConnection.source)?.title ?? pendingConnection.source : ""}</span>
                            {" \u2192 "}
                            <span className="font-medium text-foreground">{pendingConnection ? chunkMap.get(pendingConnection.target)?.title ?? pendingConnection.target : ""}</span>
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
                                className="rounded-md border-2 px-3 py-2 text-sm font-medium transition-colors hover:bg-muted disabled:opacity-50"
                                style={{ borderColor: relationColor(rel) }}
                            >
                                {rel.replace(/_/g, " ")}
                            </button>
                        ))}
                    </div>
                </DialogPopup>
            </Dialog>

            {/* Path info bar */}
            {(pathStartId || pathEndId) && (
                <div className="absolute bottom-6 left-1/2 z-10 -translate-x-1/2">
                    <div className="flex items-center gap-3 rounded-lg border bg-background/90 px-4 py-2 text-sm shadow-lg backdrop-blur-sm">
                        {pathStartId && !pathEndId && (
                            <span className="text-muted-foreground">
                                Alt+click another node to find path from <span className="font-medium text-foreground">{chunkMap.get(pathStartId)?.title ?? pathStartId}</span>
                            </span>
                        )}
                        {pathStartId && pathEndId && pathResult && (
                            <span className="text-foreground font-medium">
                                Path: {pathResult.length} {pathResult.length === 1 ? "hop" : "hops"}
                            </span>
                        )}
                        {pathStartId && pathEndId && !pathResult && (
                            <span className="font-medium text-red-500">No path found</span>
                        )}
                        <button
                            onClick={() => { setPathStartId(null); setPathEndId(null); }}
                            className="text-muted-foreground hover:text-foreground rounded border px-2 py-0.5 text-xs"
                        >
                            Clear
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
