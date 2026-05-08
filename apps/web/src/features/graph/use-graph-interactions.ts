/**
 * Search, focus, path, and selection helpers for the graph view.
 *
 * Computes derived sets (search matches, focus neighbors, path results)
 * from layout nodes/edges and interaction state.
 */

import { useMemo } from "react";
import type { Edge, Node } from "@xyflow/react";

import { findShortestPath, getNodesWithinHops } from "@/features/graph/graph-utils";

interface UseGraphInteractionsParams {
    layoutNodes: Node[];
    layoutEdges: Edge[];
    debouncedSearchQuery: string;
    focusedNodeId: string | null;
    focusModeNodeId: string | null;
    selectedChunkId: string | null;
    pathStartId: string | null;
    pathEndId: string | null;
}

export function useGraphInteractions({
    layoutNodes,
    layoutEdges,
    debouncedSearchQuery,
    focusedNodeId,
    focusModeNodeId,
    selectedChunkId,
    pathStartId,
    pathEndId,
}: UseGraphInteractionsParams) {
    // Search match IDs
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

    return {
        searchMatchIds,
        focusNeighbors,
        focusModeNeighbors,
        selectedEdgeIds,
        selectedNeighborNodes,
        pathResult,
    };
}
