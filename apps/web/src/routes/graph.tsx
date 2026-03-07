import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import "@xyflow/react/dist/style.css";
import { Background, BackgroundVariant, Controls, MiniMap, ReactFlow, ReactFlowProvider, useEdgesState, useNodesState, useReactFlow, type Edge, type Node } from "@xyflow/react";
import { useEffect, useMemo, useState } from "react";

import { relationColor } from "@/features/chunks/relation-colors";
import { FloatingEdge } from "@/features/graph/floating-edge";
import { buildQuadtree, computeRepulsion } from "@/features/graph/quadtree";
import { GraphDetailPanel } from "@/features/graph/graph-detail-panel";
import { GraphFilters } from "@/features/graph/graph-filters";
import { GraphLegend } from "@/features/graph/graph-legend";
import { GraphNode } from "@/features/graph/graph-node";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/graph")({
    component: GraphView,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest access
        }
        return { session };
    }
});

const MAIN_NODE_ID = "__main__";

const EDGE_TYPES = { floating: FloatingEdge };
const NODE_TYPES = { chunk: GraphNode };

const TYPE_COLORS: Record<string, { bg: string; border: string }> = {
    note: { bg: "#1e293b", border: "#475569" },
    guide: { bg: "#1e1b4b", border: "#6366f1" },
    reference: { bg: "#042f2e", border: "#14b8a6" },
    document: { bg: "#172554", border: "#3b82f6" },
    schema: { bg: "#1c1917", border: "#f59e0b" },
    checklist: { bg: "#1a2e05", border: "#84cc16" }
};

function GraphViewInner() {
    const [selectedChunkId, setSelectedChunkId] = useState<string | null>(null);
    const { setCenter } = useReactFlow();

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

    // Focus mode state
    const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null);

    // Collapsible clusters
    const [collapsedParents, setCollapsedParents] = useState<Set<string>>(new Set());

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

    const { layoutNodes, layoutEdges } = useMemo(() => {
        let chunks = data?.chunks ?? [];
        let connections = data?.connections ?? [];

        if (chunks.length === 0) return { layoutNodes: [] as Node[], layoutEdges: [] as Edge[] };

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
                    background: "#0f172a",
                    borderColor: "#e2e8f0",
                    borderWidth: 2,
                    borderRadius: 12,
                    color: "#f8fafc",
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
                    const scale = Math.min(1 + count * 0.05, 1.5);
                    const isParent = parentChildren.has(c.id);
                    const isChild = childIds.has(c.id);
                    const childCount = collapsedParents.has(c.id) ? parentChildren.get(c.id)?.size ?? 0 : 0;
                    const label = childCount > 0 ? `${c.title} (${childCount})` : c.title;
                    return {
                        id: c.id,
                        type: "chunk",
                        data: { label, type: c.type },
                        position: { x: 0, y: 0 },
                        style: {
                            cursor: "pointer",
                            background: typeColor!.bg,
                            borderColor: typeColor!.border,
                            borderWidth: isParent ? 2 : collapsedParents.has(c.id) ? 2.5 : 1.5,
                            borderRadius: isParent ? 12 : 10,
                            color: isChild ? "#cbd5e1" : "#e2e8f0",
                            fontSize: isParent ? 13 : 12,
                            fontWeight: isParent ? 600 : 500,
                            padding: isParent ? "10px 16px" : "8px 14px",
                            minWidth: `${Math.round((isParent ? 200 : 180) * scale)}px`,
                            letterSpacing: "0.01em"
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
                style: { stroke: color, strokeWidth: 2 }
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
                    style: { stroke: "#334155", strokeWidth: 1, strokeDasharray: "4 4" }
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

        // --- Force-directed layout ---
        const nodeCount = rawNodes.length;
        const spacing = Math.max(250, Math.sqrt(nodeCount) * 120);

        // Initialize positions in a circle to seed the simulation
        const pos = new Map<string, { x: number; y: number; vx: number; vy: number }>();
        for (let i = 0; i < rawNodes.length; i++) {
            const id = rawNodes[i]!.id;
            if (id === MAIN_NODE_ID) {
                pos.set(id, { x: 0, y: 0, vx: 0, vy: 0 });
            } else {
                const angle = (2 * Math.PI * i) / rawNodes.length;
                const r = spacing * 0.8;
                pos.set(id, { x: Math.cos(angle) * r, y: Math.sin(angle) * r, vx: 0, vy: 0 });
            }
        }

        // Build type lookup for clustering
        const nodeType = new Map<string, string>();
        for (const c of chunks) {
            if (!hiddenIds.has(c.id)) nodeType.set(c.id, c.type);
        }

        // Build edge index for spring forces
        const SPRING_LEN = 280;
        const RELATION_SPRING_LEN: Record<string, number> = {
            part_of: 180,
            depends_on: 220,
            extends: 220,
            references: 280,
            related_to: 320,
            supports: 280,
            contradicts: 350,
            alternative_to: 350
        };
        const edgePairs: [string, string, number][] = rawEdges.map(e => {
            const relation = (e.data as { relation?: string })?.relation ?? "";
            const len = RELATION_SPRING_LEN[relation] ?? SPRING_LEN;
            return [e.source, e.target, len];
        });

        const REPULSION = 80000;
        const SPRING_K = 0.003;
        const CENTER_PULL = 0.01;
        const DAMPING = 0.85;
        const ITERATIONS = 200;

        for (let iter = 0; iter < ITERATIONS; iter++) {
            const temp = 1 - iter / ITERATIONS; // cooling

            // Repulsion via Barnes-Hut quadtree
            const ids = [...pos.keys()];
            const bodies = ids.map(id => ({ id, x: pos.get(id)!.x, y: pos.get(id)!.y }));
            const tree = buildQuadtree(bodies);
            if (tree) {
                for (const id of ids) {
                    const p = pos.get(id)!;
                    const { fx, fy } = computeRepulsion(tree, p.x, p.y, REPULSION * temp);
                    p.vx += fx;
                    p.vy += fy;
                }
            }

            // Spring attraction along edges
            for (const [srcId, tgtId, targetLen] of edgePairs) {
                const a = pos.get(srcId);
                const b = pos.get(tgtId);
                if (!a || !b) continue;
                const dx = b.x - a.x;
                const dy = b.y - a.y;
                const dist = Math.sqrt(dx * dx + dy * dy) + 1;
                const force = SPRING_K * (dist - targetLen) * temp;
                const fx = (dx / dist) * force;
                const fy = (dy / dist) * force;
                a.vx += fx;
                a.vy += fy;
                b.vx -= fx;
                b.vy -= fy;
            }

            // Type clustering — pull toward type centroid
            const typeCentroids = new Map<string, { x: number; y: number; count: number }>();
            for (const [id, p] of pos) {
                const t = nodeType.get(id);
                if (!t) continue;
                const c = typeCentroids.get(t) ?? { x: 0, y: 0, count: 0 };
                c.x += p.x;
                c.y += p.y;
                c.count++;
                typeCentroids.set(t, c);
            }
            const CLUSTER_K = 0.002;
            for (const [id, p] of pos) {
                const t = nodeType.get(id);
                if (!t) continue;
                const c = typeCentroids.get(t)!;
                const cx = c.x / c.count;
                const cy = c.y / c.count;
                p.vx += (cx - p.x) * CLUSTER_K * temp;
                p.vy += (cy - p.y) * CLUSTER_K * temp;
            }

            // Center gravity
            for (const p of pos.values()) {
                p.vx -= p.x * CENTER_PULL * temp;
                p.vy -= p.y * CENTER_PULL * temp;
            }

            // Apply velocities
            for (const [id, p] of pos) {
                if (id === MAIN_NODE_ID) { p.vx = 0; p.vy = 0; continue; } // pin center
                p.x += p.vx;
                p.y += p.vy;
                p.vx *= DAMPING;
                p.vy *= DAMPING;
            }
        }

        const layoutNodes = rawNodes.map(node => {
            const dragged = draggedPositions.get(node.id);
            if (dragged) return { ...node, position: dragged };
            const p = pos.get(node.id) ?? { x: 0, y: 0 };
            return { ...node, position: { x: p.x - 100, y: p.y - 25 } };
        });

        return { layoutNodes, layoutEdges: rawEdges };
    }, [data, filterTypes, filterRelations, collapsedParents, draggedPositions]);

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

    const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
    const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
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
        if (!searchQuery.trim()) {
            setNodes(layoutNodes);
            setEdges(layoutEdges);
            return;
        }
        const q = searchQuery.toLowerCase();
        const matchIds = new Set<string>();
        for (const node of layoutNodes) {
            if (node.id === MAIN_NODE_ID) continue;
            const label = typeof node.data.label === "string" ? node.data.label : "";
            if (label.toLowerCase().includes(q)) matchIds.add(node.id);
        }
        setNodes(
            layoutNodes.map(node => {
                if (node.id === MAIN_NODE_ID) return node;
                return {
                    ...node,
                    style: {
                        ...(node.style as Record<string, unknown>),
                        opacity: matchIds.has(node.id) ? 1 : 0.15,
                        transition: "opacity 0.2s"
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
    }, [searchQuery, layoutNodes, layoutEdges, setNodes, setEdges]);

    // Apply focus dimming
    useEffect(() => {
        if (!focusNeighbors || searchQuery.trim()) return;
        setNodes(
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
    }, [focusNeighbors, layoutNodes, layoutEdges, setNodes, setEdges, searchQuery]);

    // Sync layout when search is empty and no focus
    useEffect(() => {
        if (!searchQuery.trim() && !focusedNodeId) {
            if (selectedNeighborNodes) {
                setNodes(layoutNodes.map(node => ({
                    ...node,
                    style: {
                        ...(node.style as Record<string, unknown>),
                        opacity: selectedNeighborNodes.has(node.id) ? 1 : 0.2,
                        transition: "opacity 0.2s"
                    }
                })));
            } else {
                setNodes(layoutNodes);
            }
        }
    }, [layoutNodes, setNodes, searchQuery, focusedNodeId, selectedNeighborNodes]);

    useEffect(() => {
        if (!searchQuery.trim() && !focusedNodeId) {
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
    }, [layoutEdges, setEdges, searchQuery, focusedNodeId, selectedEdgeIds]);

    // eslint-disable-next-line react-hooks/exhaustive-deps
    useEffect(() => {
        if (!selectedChunkId) return;
        const node = nodes.find(n => n.id === selectedChunkId);
        if (!node) return;
        const x = node.position.x + ((node.measured?.width ?? 180) / 2);
        const y = node.position.y + ((node.measured?.height ?? 40) / 2);
        setCenter(x, y, { duration: 400 });
    }, [selectedChunkId]);

    if (isLoading) {
        return (
            <div className="flex h-[calc(100vh-4rem)] items-center justify-center">
                <p className="text-muted-foreground">Loading graph...</p>
            </div>
        );
    }

    return (
        <div className="flex h-[calc(100vh-4rem)]">
            <div
                className={`shrink-0 overflow-hidden transition-[width] duration-300 ease-in-out ${selectedChunkId ? "w-[380px]" : "w-0"}`}
            >
                {selectedChunkId && (
                    <div className="w-[380px]">
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
            </div>
            <div className="relative flex-1 [&_.react-flow__handle]:invisible [&_.react-flow__node]:transition-[transform] [&_.react-flow__node]:duration-500 [&_.react-flow__node]:ease-out">
            <ReactFlow
                nodes={nodes}
                edges={edges}
                nodeTypes={NODE_TYPES}
                edgeTypes={EDGE_TYPES}
                onNodesChange={onNodesChange}
                onEdgesChange={onEdgesChange}
                onNodeClick={(_, node) => {
                    if (node.id === MAIN_NODE_ID) return;
                    if (focusedNodeId === node.id) {
                        setSelectedChunkId(node.id);
                    } else {
                        setFocusedNodeId(node.id);
                    }
                }}
                onPaneClick={() => {
                    setFocusedNodeId(null);
                    setSelectedChunkId(null);
                }}
                onNodeDoubleClick={(_, node) => {
                    if (node.id === MAIN_NODE_ID) return;
                    setCollapsedParents(prev => {
                        const next = new Set(prev);
                        if (next.has(node.id)) next.delete(node.id);
                        else next.add(node.id);
                        return next;
                    });
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
                onEdgeMouseEnter={(_, edge) => {
                    setEdges(es =>
                        es.map(e =>
                            e.id === edge.id
                                ? {
                                      ...e,
                                      label: (e.data as { relation?: string })?.relation,
                                      labelStyle: { fill: (e.style as Record<string, string>)?.stroke, fontSize: 10, fontWeight: 500 },
                                      labelBgStyle: { fill: "rgba(0,0,0,0.6)", fillOpacity: 0.8 }
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
                colorMode="dark"
            >
                <Background variant={BackgroundVariant.Dots} gap={20} size={1} color="rgba(148,163,184,0.15)" />
                <Controls />
                <MiniMap
                    nodeColor={node => {
                        if (node.id === MAIN_NODE_ID) return "#e2e8f0";
                        const style = node.style as Record<string, string> | undefined;
                        return style?.borderColor ?? "#475569";
                    }}
                    maskColor="rgba(0, 0, 0, 0.7)"
                    pannable
                    zoomable
                />
            </ReactFlow>

            {/* Top-left: Filters */}
            <GraphFilters
                types={allTypes}
                relations={allRelations}
                activeTypes={filterTypes}
                activeRelations={filterRelations}
                onToggleType={toggleType}
                onToggleRelation={toggleRelation}
            />

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
                <span className="text-muted-foreground rounded-lg border bg-background/80 px-3 py-1.5 text-xs backdrop-blur-sm">
                    {nodes.length - 1} · {edges.length}
                </span>
            </div>

            {/* Bottom-left: Legend */}
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
            </div>
        </div>
    );
}

function GraphView() {
    return (
        <ReactFlowProvider>
            <GraphViewInner />
        </ReactFlowProvider>
    );
}
