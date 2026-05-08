/**
 * Layout engine for the graph view.
 *
 * Manages the web worker lifecycle, computes filtered graph data, runs
 * force-directed / hierarchical / radial layouts, and caches results.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useReactFlow } from "@xyflow/react";

import { applyPrefilter } from "@/features/graph/apply-prefilter";
import { runForceLayout, runRegroupLayout } from "@/features/graph/force-layout";
import { getCachedLayout, layoutCacheKey, setCachedLayout } from "@/features/graph/layout-cache";
import { mark, measure, isDev } from "@/features/graph/graph-timings";
import type { GraphFilterValues } from "@/features/graph/graph-filter-dialog";
import { hierarchicalLayout, radialLayout, type LayoutAlgorithm } from "@/features/graph/layouts";
import type { LayoutWorkerInput, LayoutWorkerOutput } from "@/features/graph/layout.worker";
import { getMostConnected } from "@/features/graph/graph-utils";
import type { GraphData } from "@/features/graph/use-graph-data";

export interface FilteredGraph {
    chunks: GraphData["chunks"];
    connections: NonNullable<GraphData["connections"]>;
    parentChildren: Map<string, Set<string>>;
    childIds: Set<string>;
    hiddenIds: Set<string>;
}

interface UseGraphLayoutParams {
    data: GraphData | undefined;
    scopedChunkTags: GraphData["chunkTags"];
    prefilter: GraphFilterValues;
    filterTypes: Set<string>;
    filterRelations: Set<string>;
    collapsedParents: Set<string>;
    exploreMode: boolean;
    exploredNodeIds: Set<string>;
    timelineCutoff: Date | null;
    layoutAlgorithm: LayoutAlgorithm;
    useMainThread: boolean;
    selectedChunkId: string | null;
    tagGroups: Map<string, string[]> | null;
    groupingMode: string | null;
}

export function useGraphLayout({
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
}: UseGraphLayoutParams) {
    const { fitView: fitViewRaw } = useReactFlow();
    const fitViewRef = useRef(fitViewRaw);
    fitViewRef.current = fitViewRaw;
    const fitView = useCallback((...args: Parameters<typeof fitViewRaw>) => fitViewRef.current(...args), []);

    // Stabilize tagGroups reference — only change when content changes
    const tagGroupsRef = useRef(tagGroups);
    const tagGroupsKey = useMemo(() => {
        if (!tagGroups) return "";
        return [...tagGroups.entries()].map(([k, v]) => `${k}:${v.length}`).sort().join(";");
    }, [tagGroups]);
    if (tagGroups !== tagGroupsRef.current) {
        const prevKey = tagGroupsRef.current
            ? [...tagGroupsRef.current.entries()].map(([k, v]) => `${k}:${v.length}`).sort().join(";")
            : "";
        if (tagGroupsKey !== prevKey) tagGroupsRef.current = tagGroups;
    }
    const stableTagGroups = tagGroupsRef.current;

    // --- Web Worker for force-directed layout ---
    const workerRef = useRef<Worker | null>(null);
    const requestIdRef = useRef<number>(0);
    const [layoutPositions, setLayoutPositions] = useState<Record<string, { x: number; y: number }> | null>(null);
    const [isLayouting, setIsLayouting] = useState(false);

    // Signature of the last full simulation
    const lastSimulationSigRef = useRef<string | null>(null);
    const lastSimulationPositionsRef = useRef<Record<string, { x: number; y: number }> | null>(null);
    const pendingCacheKeyRef = useRef<string | null>(null);

    // Mount-time marker
    useEffect(() => {
        mark("mount");
    }, []);

    // Create / teardown the worker
    useEffect(() => {
        const worker = new Worker(new URL("./layout.worker.ts", import.meta.url), { type: "module" });
        worker.onmessage = (e: MessageEvent<LayoutWorkerOutput>) => {
            if (e.data.requestId !== requestIdRef.current) return;
            mark("layout-end");
            measure("layout-duration", "layout-start", "layout-end");
            setLayoutPositions(e.data.positions);
            lastSimulationPositionsRef.current = e.data.positions;
            if (pendingCacheKeyRef.current) {
                setCachedLayout(pendingCacheKeyRef.current, e.data.positions);
                pendingCacheKeyRef.current = null;
            }
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
    const filteredGraph = useMemo((): FilteredGraph | null => {
        if (!data?.chunks || data.chunks.length === 0) return null;

        const prefiltered = applyPrefilter(
            { chunks: data.chunks, connections: data.connections ?? [], chunkTags: scopedChunkTags },
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

        if (isDev) console.debug("[graph] filteredGraph: %d chunks, %d connections", chunks.length, connections.length);
        return { chunks, connections, parentChildren, childIds, hiddenIds };
    }, [data, filterTypes, filterRelations, collapsedParents, exploreMode, exploredNodeIds, timelineCutoff, prefilter, scopedChunkTags]);

    // Post to worker when simulation inputs change
    useEffect(() => {
        if (!filteredGraph) return;

        const { chunks, connections, hiddenIds } = filteredGraph;

        const workerNodes: LayoutWorkerInput["nodes"] = [
            ...chunks.filter(c => !hiddenIds.has(c.id)).map(c => ({ id: c.id, type: c.type }))
        ];

        const workerEdges: LayoutWorkerInput["edges"] = connections.map(conn => ({
            source: conn.sourceId,
            target: conn.targetId,
            relation: conn.relation
        }));

        const sortedNodeIds = workerNodes.map(n => n.id).sort();
        const sortedEdgeIds = workerEdges.map(e => `${e.source}>${e.target}:${e.relation}`).sort();
        const nodeIds = sortedNodeIds.join("|");
        const edgeIds = sortedEdgeIds.join("|");
        const sig = `${layoutAlgorithm}::${nodeIds}::${edgeIds}`;
        const dataUnchanged = sig === lastSimulationSigRef.current;
        const prevPositions = lastSimulationPositionsRef.current;

        const groupingKey = groupingMode ?? "none";
        const tagGroupMembership = stableTagGroups
            ? [...stableTagGroups.entries()]
                  .map(([name, ids]) => `${name}:${[...ids].sort().join(",")}`)
                  .sort()
                  .join(";")
            : "";
        const cacheKey = layoutCacheKey({
            layoutAlgorithm,
            groupingMode: `${groupingKey}#${tagGroupMembership}`,
            nodeIds: sortedNodeIds,
            edgeIds: sortedEdgeIds
        });

        mark("layout-start");
        if (isDev) console.debug("[graph] layout: %d nodes, %d edges, grouping=%s, groups=%d, dataUnchanged=%s",
            workerNodes.length, workerEdges.length, groupingKey, stableTagGroups?.size ?? 0, dataUnchanged);

        const cached = getCachedLayout(cacheKey);
        if (cached && layoutAlgorithm === "force") {
            mark("layout-end");
            measure("layout-cache-hit", "layout-start", "layout-end");
            if (isDev) console.debug("[graph] layout: cache hit, %d positions", Object.keys(cached).length);
            setLayoutPositions(cached);
            lastSimulationSigRef.current = sig;
            lastSimulationPositionsRef.current = cached;
            setIsLayouting(false);
            setTimeout(() => fitView({ padding: 0.1 }), 100);
            return;
        }
        if (layoutAlgorithm === "force") {
            if (dataUnchanged && prevPositions && stableTagGroups && stableTagGroups.size > 0) {
                if (isDev) console.debug("[graph] layout: trying regroup fast path");
                const regroupedPositions = runRegroupLayout(workerNodes, workerEdges, stableTagGroups, prevPositions);
                if (regroupedPositions) {
                    mark("layout-end");
                    measure("layout-regroup", "layout-start", "layout-end");
                    if (isDev) console.debug("[graph] layout: regroup success, %d positions", Object.keys(regroupedPositions).length);
                    setLayoutPositions(regroupedPositions);
                    lastSimulationPositionsRef.current = regroupedPositions;
                    setCachedLayout(cacheKey, regroupedPositions);
                    setIsLayouting(false);
                    setTimeout(() => fitView({ padding: 0.1 }), 100);
                    return;
                }
            }

            const tagGroupsObj = stableTagGroups ? Object.fromEntries(stableTagGroups) : undefined;
            if (useMainThread) {
                setIsLayouting(true);
                const positions = runForceLayout(workerNodes, workerEdges, stableTagGroups ?? undefined);
                mark("layout-end");
                measure("layout-duration", "layout-start", "layout-end");
                setLayoutPositions(positions);
                lastSimulationSigRef.current = sig;
                lastSimulationPositionsRef.current = positions;
                setCachedLayout(cacheKey, positions);
                setIsLayouting(false);
            } else {
                if (!workerRef.current) return;
                setIsLayouting(true);
                requestIdRef.current += 1;
                pendingCacheKeyRef.current = cacheKey;
                workerRef.current.postMessage({
                    requestId: requestIdRef.current,
                    nodes: workerNodes,
                    edges: workerEdges,
                    tagGroups: tagGroupsObj
                } satisfies LayoutWorkerInput);
                lastSimulationSigRef.current = sig;
            }
        } else {
            let positions: Record<string, { x: number; y: number }>;
            if (layoutAlgorithm === "hierarchical") {
                positions = hierarchicalLayout(workerNodes, workerEdges);
            } else {
                const center = selectedChunkId ?? getMostConnected(workerNodes, workerEdges);
                positions = center ? radialLayout(workerNodes, workerEdges, center) : hierarchicalLayout(workerNodes, workerEdges);
            }
            mark("layout-end");
            measure("layout-duration", "layout-start", "layout-end");
            setLayoutPositions(positions);
            setIsLayouting(false);
        }
    }, [filteredGraph, layoutAlgorithm, useMainThread, selectedChunkId, stableTagGroups, groupingMode, fitView]);

    return {
        filteredGraph,
        layoutPositions,
        isLayouting,
    };
}
