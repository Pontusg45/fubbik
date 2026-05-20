/**
 * Zoom-level-aware node and edge builder for the graph view.
 *
 * At overview level: builds island nodes + bridge edges.
 * At neighborhood/detail level: builds chunk cards (1-hop), chunk dots (2-hop), typed edges.
 */

import { useMemo } from "react";
import type { Edge, Node } from "@xyflow/react";

import { relationColor } from "@/features/chunks/relation-colors";
import type { Island, IslandBridge } from "@/features/graph/island-formation";
import type { NeighborhoodResult } from "@/features/graph/neighborhood-layout";
import type { ZoomLevel } from "@/features/graph/use-graph-zoom";
import type { GraphData } from "@/features/graph/use-graph-data";

interface UseGraphNodesParams {
    zoomLevel: ZoomLevel;
    islands: Island[];
    bridges: IslandBridge[];
    islandPositions: Record<string, { x: number; y: number }>;
    islandHealthScores: Map<string, number[]>;
    islandColors: Map<string, string>;
    neighborhood: NeighborhoodResult | null;
    focusChunkId: string | null;
    data: GraphData | undefined;
    chunkSummaries: Map<string, string | null>;
    chunkHealthScores: Map<string, number>;
    chunkTags: Map<string, Array<{ name: string; color: string }>>;
    filterTypes: Set<string>;
    filterRelations: Set<string>;
    heatmapMode: boolean;
}

export function useGraphNodes({
    zoomLevel,
    islands,
    bridges,
    islandPositions,
    islandHealthScores,
    islandColors,
    neighborhood,
    focusChunkId,
    data,
    chunkSummaries,
    chunkHealthScores,
    chunkTags,
    filterTypes,
    filterRelations,
    heatmapMode: _heatmapMode,
}: UseGraphNodesParams): { layoutNodes: Node[]; layoutEdges: Edge[] } {
    // Overview: island nodes + bridge edges
    const overviewResult = useMemo(() => {
        if (zoomLevel !== "overview") return null;

        const nodes: Node[] = islands.map(island => {
            const pos = islandPositions[island.id] ?? { x: 0, y: 0 };
            const healthScores = islandHealthScores.get(island.id) ?? [];
            const color = islandColors.get(island.id) ?? "#6b7280";
            return {
                id: `island-${island.id}`,
                type: "island",
                position: pos,
                data: {
                    name: island.name,
                    chunkCount: island.chunkIds.length,
                    color,
                    healthScores,
                    isSingleton: island.isSingleton,
                },
            };
        });

        const edges: Edge[] = bridges.map((bridge, i) => {
            const color = relationColor(bridge.dominantRelation);
            return {
                id: `bridge-${i}`,
                source: `island-${bridge.fromIslandId}`,
                target: `island-${bridge.toIslandId}`,
                type: "typed",
                data: { relation: bridge.dominantRelation },
                style: {
                    stroke: color,
                    strokeWidth: Math.min(1 + bridge.count, 5),
                },
                label: bridge.count > 1 ? String(bridge.count) : undefined,
            };
        });

        return { layoutNodes: nodes, layoutEdges: edges };
    }, [zoomLevel, islands, bridges, islandPositions, islandHealthScores, islandColors]);

    // Neighborhood/detail: chunk cards + dots + typed edges
    const neighborhoodResult = useMemo(() => {
        if (zoomLevel === "overview" || !neighborhood) return null;

        const chunkMap = new Map<string, { id: string; title: string; type: string; summary: string | null }>();
        for (const c of data?.chunks ?? []) {
            chunkMap.set(c.id, { id: c.id, title: c.title, type: c.type, summary: c.summary });
        }

        const nodes: Node[] = [];

        for (const [chunkId, hop] of neighborhood.hops) {
            const chunk = chunkMap.get(chunkId);
            if (!chunk) continue;

            // Apply type filter
            if (filterTypes.size > 0 && !filterTypes.has(chunk.type)) continue;

            const pos = neighborhood.positions[chunkId] ?? { x: 0, y: 0 };
            const isFocus = chunkId === focusChunkId;

            if (hop <= 1) {
                // Chunk card for focus + 1-hop
                nodes.push({
                    id: chunkId,
                    type: "chunkCard",
                    position: pos,
                    data: {
                        title: chunk.title,
                        summary: chunkSummaries.get(chunkId) ?? chunk.summary,
                        type: chunk.type,
                        tags: chunkTags.get(chunkId) ?? [],
                        healthScore: chunkHealthScores.get(chunkId) ?? 0,
                        isFocus,
                    },
                });
            } else {
                // Chunk dot for 2-hop
                nodes.push({
                    id: chunkId,
                    type: "chunkDot",
                    position: pos,
                    data: {
                        title: chunk.title,
                        chunkType: chunk.type,
                        healthScore: chunkHealthScores.get(chunkId) ?? 50,
                    },
                });
            }
        }

        // Build edges from visible connections
        const visibleNodeIds = new Set(nodes.map(n => n.id));
        const filteredConnections = neighborhood.visibleConnections.filter(conn => {
            if (!visibleNodeIds.has(conn.sourceId) || !visibleNodeIds.has(conn.targetId)) return false;
            if (filterRelations.size > 0 && !filterRelations.has(conn.relation)) return false;
            return true;
        });

        const edges: Edge[] = filteredConnections.map((conn, i) => {
            const color = relationColor(conn.relation);
            return {
                id: `edge-${conn.sourceId}-${conn.targetId}-${i}`,
                source: conn.sourceId,
                target: conn.targetId,
                type: "typed",
                data: { relation: conn.relation },
                style: {
                    stroke: color,
                    strokeWidth: 2,
                },
            };
        });

        return { layoutNodes: nodes, layoutEdges: edges };
    }, [zoomLevel, neighborhood, focusChunkId, data, chunkSummaries, chunkHealthScores, chunkTags, filterTypes, filterRelations]);

    if (zoomLevel === "overview" && overviewResult) {
        return overviewResult;
    }
    if (neighborhoodResult) {
        return neighborhoodResult;
    }

    return { layoutNodes: [], layoutEdges: [] };
}
