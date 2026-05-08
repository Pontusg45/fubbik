/**
 * Node and edge builder for the graph view.
 *
 * Transforms layout positions + grouping data into React Flow nodes and edges,
 * including group bounding boxes, cluster nodes, respaced grid positions,
 * edge bundling, and legend counts.
 */

import { useMemo, useRef, useState } from "react";
import type { Edge, Node } from "@xyflow/react";

import { relationColor } from "@/features/chunks/relation-colors";
import { GROUP_NODE_ID_PREFIX, UNGROUPED_NODE_ID } from "@/features/graph/group-strategies";
import type { GroupStrategyResult } from "@/features/graph/group-strategies";
import type { ClusterNode } from "@/features/graph/cluster-strategy";
import { isDev } from "@/features/graph/graph-timings";
import type { FilteredGraph } from "@/features/graph/use-graph-layout";
import type { GraphData } from "@/features/graph/use-graph-data";
import type { GraphAction } from "@/features/graph/use-graph-state";

interface UseGraphNodesParams {
    filteredGraph: FilteredGraph | null;
    layoutPositions: Record<string, { x: number; y: number }> | null;
    draggedPositions: Map<string, { x: number; y: number }>;
    isDark: boolean;
    TYPE_COLORS: Record<string, { bg: string; border: string }>;
    collapsedParents: Set<string>;
    bundleEdges: boolean;
    tagGroups: Map<string, string[]> | null;
    groupResult: GroupStrategyResult | null;
    data: GraphData | undefined;
    edgeAnimated: boolean;
    showUngrouped: boolean;
    shouldCluster: boolean;
    clusters: ClusterNode[];
    clusterVisibleChunkIds: Set<string> | null;
    dispatch: React.Dispatch<GraphAction>;
}

export function useGraphNodes({
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
}: UseGraphNodesParams) {
    // Measured node sizes for group bounding box calculation (ref + counter to trigger recalc)
    const measuredNodeSizesRef = useRef(new Map<string, { w: number; h: number }>());
    const [measuredSizesVersion, setMeasuredSizesVersion] = useState(0);

    // Live counts by type + relation for the legend
    const legendTypeCounts = useMemo(() => {
        const m = new Map<string, number>();
        for (const c of filteredGraph?.chunks ?? []) {
            m.set(c.type, (m.get(c.type) ?? 0) + 1);
        }
        return m;
    }, [filteredGraph]);

    const legendRelationCounts = useMemo(() => {
        const m = new Map<string, number>();
        for (const conn of filteredGraph?.connections ?? []) {
            m.set(conn.relation, (m.get(conn.relation) ?? 0) + 1);
        }
        return m;
    }, [filteredGraph]);

    // Build layoutNodes and layoutEdges from positions
    const { layoutNodes, layoutEdges, groupToChunkIds } = useMemo(() => {
        if (!filteredGraph) return { layoutNodes: [] as Node[], layoutEdges: [] as Edge[], groupToChunkIds: new Map<string, string[]>() };

        const { chunks, connections, parentChildren, childIds, hiddenIds } = filteredGraph;

        // Build chunk->codebaseId map for cross-codebase edge styling + node labels
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
                .filter(c => !hiddenIds.has(c.id) && (!shouldCluster || !clusterVisibleChunkIds || clusterVisibleChunkIds.has(c.id)))
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

        // Add cluster nodes when clustering is active
        if (shouldCluster && clusters.length > 0) {
            for (const cluster of clusters) {
                if (!cluster.expanded) {
                    rawNodes.push({
                        id: cluster.id,
                        type: "cluster",
                        data: {
                            label: cluster.groupName,
                            count: cluster.count,
                            color: cluster.color ?? (isDark ? "#475569" : "#94a3b8"),
                            expanded: cluster.expanded,
                            onToggle: () => dispatch({ type: "TOGGLE_CLUSTER", groupName: cluster.groupName }),
                        },
                        position: { x: 0, y: 0 },
                        selectable: false,
                        draggable: true,
                        style: { cursor: "pointer" },
                    });
                }
            }
        }

        // When clustering, filter out edges to/from non-visible chunks
        const visibleConnections = shouldCluster && clusterVisibleChunkIds
            ? connections.filter(c => clusterVisibleChunkIds.has(c.sourceId) && clusterVisibleChunkIds.has(c.targetId))
            : connections;

        const rawEdges: Edge[] = visibleConnections.map(conn => {
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

        // Build tag group nodes (visual overlay)
        const tagGroupNodeIds = new Set<string>();
        const chunkToGroupId = new Map<string, string>();

        if (tagGroups && tagGroups.size > 0) {
            const colorFor = groupResult?.colorFor;

            // Build set of visible chunk IDs (nodes already in rawNodes)
            const visibleChunkIds = new Set(rawNodes.map(n => n.id));

            for (const [label, chunkIds] of tagGroups) {
                const visibleInGroup = chunkIds.filter(cid => visibleChunkIds.has(cid));
                if (visibleInGroup.length === 0) continue;

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
                if (isDev) console.debug("[graph] hiding %d ungrouped nodes (grouped: %d)", ungroupedChunkIds.size, groupedChunkIds.size);
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
        const GRID_GAP_X = 16;
        const GRID_GAP_Y = 12;
        const groupBounds = new Map<string, { minX: number; minY: number; maxX: number; maxY: number }>();

        const measuredSizes = measuredNodeSizesRef.current;

        // Re-space grid positions using measured node widths/heights
        const respacedPositions = new Map<string, { x: number; y: number }>();
        if (tagGroupNodeIds.size > 0 && layoutPositions) {
            const groupMembers = new Map<string, string[]>();
            for (const [chunkId, groupId] of chunkToGroupId) {
                if (!groupMembers.has(groupId)) groupMembers.set(groupId, []);
                groupMembers.get(groupId)!.push(chunkId);
            }

            function colsForCount(n: number) {
                if (n <= 1) return 1;
                const NODE_W = 200, NODE_H = 72;
                return Math.max(1, Math.ceil(Math.sqrt(n * (NODE_H / NODE_W))));
            }

            for (const [, members] of groupMembers) {
                if (members.length === 0) continue;
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

            // Build bounds from the re-spaced positions
            for (const [chunkId, groupId] of chunkToGroupId) {
                const dragged = draggedPositions.get(chunkId);
                const re = respacedPositions.get(chunkId);
                const lp = layoutPositions[chunkId];
                if (!dragged && !re && !lp) continue;
                const size = measuredSizes.get(chunkId);
                const w = size?.w ?? DEFAULT_NODE_W;
                const h = size?.h ?? DEFAULT_NODE_H;
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

        // Compute grid positions for collapsed cluster nodes
        const clusterPositions = new Map<string, { x: number; y: number }>();
        if (shouldCluster) {
            const collapsedClusters = clusters.filter(c => !c.expanded);
            const cols = Math.max(1, Math.ceil(Math.sqrt(collapsedClusters.length)));
            const CLUSTER_GAP_X = 220;
            const CLUSTER_GAP_Y = 140;
            for (let i = 0; i < collapsedClusters.length; i++) {
                const col = i % cols;
                const row = Math.floor(i / cols);
                clusterPositions.set(collapsedClusters[i]!.id, { x: col * CLUSTER_GAP_X, y: row * CLUSTER_GAP_Y });
            }
        }

        // Apply positions from worker (or fallback to origin)
        const layoutNodes = rawNodes.map(node => {
            const p = layoutPositions?.[node.id] ?? { x: 0, y: 0 };

            // Cluster nodes: use grid position
            if (node.type === "cluster") {
                const dragged = draggedPositions.get(node.id);
                const gridPos = clusterPositions.get(node.id) ?? { x: 0, y: 0 };
                return { ...node, position: dragged ?? gridPos };
            }

            // Group nodes: compute size from child bounds.
            if (tagGroupNodeIds.has(node.id)) {
                const bounds = groupBounds.get(node.id);
                if (!bounds) {
                    return { ...node, position: { x: -9999, y: -9999 }, style: { ...node.style, width: 0, height: 0, opacity: 0 } };
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
            const re = respacedPositions.get(node.id);
            const center = re ?? p;
            return { ...node, position: { x: center.x - hw, y: center.y - hh } };
        });

        if (bundleEdges) {
            const nodeTypeMap = new Map<string, string>();
            for (const c of filteredGraph.chunks) {
                nodeTypeMap.set(c.id, c.type);
            }

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

        if (isDev) console.debug("[graph] render: %d nodes (%d groups), %d edges",
            layoutNodes.length, layoutNodes.filter(n => n.type === "group").length, rawEdges.length);
        return { layoutNodes, layoutEdges: rawEdges, groupToChunkIds };
    }, [filteredGraph, layoutPositions, draggedPositions, isDark, TYPE_COLORS, collapsedParents, bundleEdges, tagGroups, data, edgeAnimated, measuredSizesVersion, showUngrouped, shouldCluster, clusters, clusterVisibleChunkIds, dispatch, groupResult]);

    return {
        layoutNodes,
        layoutEdges,
        groupToChunkIds,
        legendTypeCounts,
        legendRelationCounts,
        measuredNodeSizesRef,
        setMeasuredSizesVersion,
    };
}
