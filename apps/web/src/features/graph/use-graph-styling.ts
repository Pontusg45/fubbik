/**
 * Consolidated node/edge styling effect for the graph view.
 *
 * Single source of truth for visual state: search highlight, focus dim,
 * path highlight, selection opacity, tag-group opacity, and multi-select outlines.
 */

import { useEffect } from "react";
import type { Edge, Node } from "@xyflow/react";

import { isGroupNodeId } from "@/features/graph/group-strategies";

interface UseGraphStylingParams {
    layoutNodes: Node[];
    layoutEdges: Edge[];
    debouncedSearchQuery: string;
    focusModeNeighbors: Set<string> | null;
    focusNeighbors: Set<string> | null;
    selectedNeighborNodes: Set<string> | null;
    selectedEdgeIds: Set<string> | null;
    multiSelectedIds: Set<string>;
    pathResult: { pathNodeIds: Set<string>; pathEdgeIds: Set<string> } | null;
    chunkTagGroupMap: Map<string, Set<string>> | null;
    selectedChunkId: string | null;
    mergeNodes: (nodes: Node[]) => void;
    setEdges: React.Dispatch<React.SetStateAction<Edge[]>>;
}

export function useGraphStyling({
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
}: UseGraphStylingParams) {
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
}
