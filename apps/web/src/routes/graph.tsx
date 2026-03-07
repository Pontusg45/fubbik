import { useQuery } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import "@xyflow/react/dist/style.css";
import { Background, BackgroundVariant, Controls, MiniMap, ReactFlow, useEdgesState, useNodesState, type Edge, type Node } from "@xyflow/react";
import { useEffect, useMemo, useState } from "react";

import { relationColor } from "@/features/chunks/relation-colors";
import { FloatingEdge } from "@/features/graph/floating-edge";
import { GraphFilters } from "@/features/graph/graph-filters";
import { GraphLegend } from "@/features/graph/graph-legend";
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

const TYPE_COLORS: Record<string, { bg: string; bgGrad: string; border: string; glow: string }> = {
    note: { bg: "#1e293b", bgGrad: "#253347", border: "#475569", glow: "rgba(71,85,105,0.3)" },
    guide: { bg: "#1e1b4b", bgGrad: "#272358", border: "#6366f1", glow: "rgba(99,102,241,0.35)" },
    reference: { bg: "#042f2e", bgGrad: "#0a3d3b", border: "#14b8a6", glow: "rgba(20,184,166,0.3)" },
    document: { bg: "#172554", bgGrad: "#1e3268", border: "#3b82f6", glow: "rgba(59,130,246,0.35)" },
    schema: { bg: "#1c1917", bgGrad: "#2a2520", border: "#f59e0b", glow: "rgba(245,158,11,0.3)" },
    checklist: { bg: "#1a2e05", bgGrad: "#243c0a", border: "#84cc16", glow: "rgba(132,204,22,0.3)" }
};

function GraphView() {
    const navigate = useNavigate();

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

    // Collapsible clusters
    const [collapsedParents, setCollapsedParents] = useState<Set<string>>(new Set());

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
                    background: "linear-gradient(145deg, #1a2744, #0c1425)",
                    borderColor: "#e2e8f0",
                    borderWidth: 2,
                    borderRadius: 12,
                    color: "#f8fafc",
                    fontSize: 14,
                    fontWeight: 600,
                    padding: "10px 18px",
                    boxShadow: "0 4px 20px rgba(226,232,240,0.15), 0 0 40px rgba(226,232,240,0.05)"
                }
            },
            ...chunks
                .filter(c => !hiddenIds.has(c.id))
                .map(c => {
                    const typeColor = TYPE_COLORS[c.type] ?? TYPE_COLORS.note;
                    const count = connectionCounts.get(c.id) ?? 0;
                    const scale = Math.min(1 + count * 0.05, 1.5);
                    const childCount = collapsedParents.has(c.id) ? parentChildren.get(c.id)?.size ?? 0 : 0;
                    const label = childCount > 0 ? `${c.title} (${childCount})` : c.title;
                    return {
                        id: c.id,
                        data: { label },
                        position: { x: 0, y: 0 },
                        style: {
                            cursor: "pointer",
                            background: `linear-gradient(145deg, ${typeColor!.bgGrad}, ${typeColor!.bg})`,
                            borderColor: typeColor!.border,
                            borderWidth: collapsedParents.has(c.id) ? 2.5 : 1.5,
                            borderRadius: 10,
                            color: "#e2e8f0",
                            fontSize: 12,
                            fontWeight: 500,
                            padding: "8px 14px",
                            minWidth: `${Math.round(180 * scale)}px`,
                            boxShadow: `0 2px 12px ${typeColor!.glow}, inset 0 1px 0 rgba(255,255,255,0.06)`,
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

            // Repulsion between all pairs
            const ids = [...pos.keys()];
            for (let i = 0; i < ids.length; i++) {
                const a = pos.get(ids[i]!)!;
                for (let j = i + 1; j < ids.length; j++) {
                    const b = pos.get(ids[j]!)!;
                    let dx = a.x - b.x;
                    let dy = a.y - b.y;
                    const distSq = dx * dx + dy * dy + 1;
                    const force = (REPULSION * temp) / distSq;
                    const dist = Math.sqrt(distSq);
                    dx /= dist;
                    dy /= dist;
                    a.vx += dx * force;
                    a.vy += dy * force;
                    b.vx -= dx * force;
                    b.vy -= dy * force;
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
            const p = pos.get(node.id) ?? { x: 0, y: 0 };
            return { ...node, position: { x: p.x - 100, y: p.y - 25 } };
        });

        return { layoutNodes, layoutEdges: rawEdges };
    }, [data, filterTypes, filterRelations, collapsedParents]);

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

    // Sync layout when search is empty
    useEffect(() => {
        if (!searchQuery.trim()) {
            setNodes(layoutNodes);
        }
    }, [layoutNodes, setNodes, searchQuery]);

    useEffect(() => {
        if (!searchQuery.trim()) {
            setEdges(layoutEdges);
        }
    }, [layoutEdges, setEdges, searchQuery]);

    if (isLoading) {
        return (
            <div className="flex h-[calc(100vh-4rem)] items-center justify-center">
                <p className="text-muted-foreground">Loading graph...</p>
            </div>
        );
    }

    return (
        <div className="relative h-[calc(100vh-4rem)] [&_.react-flow__handle]:invisible">
            <ReactFlow
                nodes={nodes}
                edges={edges}
                edgeTypes={EDGE_TYPES}
                onNodesChange={onNodesChange}
                onEdgesChange={onEdgesChange}
                onNodeClick={(_, node) => {
                    if (node.id === MAIN_NODE_ID) return;
                    navigate({ to: "/chunks/$chunkId", params: { chunkId: node.id } });
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
    );
}
