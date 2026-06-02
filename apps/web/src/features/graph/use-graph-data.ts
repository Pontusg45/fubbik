/**
 * Data fetching, scoping, and island computation for the graph view.
 *
 * Owns the graph API query, derives scoped chunk-tags + available tag-type IDs,
 * and computes island formation from the selected grouping tag type.
 */

import { useQuery } from "@tanstack/react-query";
import { useSearch } from "@tanstack/react-router";
import { useEffect, useMemo, useState } from "react";

import { formIslands, type IslandFormationResult } from "@/features/graph/island-formation";
import type { GraphAction } from "@/features/graph/use-graph-state";
import { useActiveSpace } from "@/features/spaces/use-active-space";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export type GraphData = NonNullable<ReturnType<typeof useGraphData>["data"]>;

export function useGraphData(dispatch: React.Dispatch<GraphAction>) {
    const { spaceId, workspaceId } = useActiveSpace();

    const { data, isLoading } = useQuery({
        queryKey: ["graph", spaceId, workspaceId],
        queryFn: async () => {
            return unwrapEden(
                await api.api.graph.get({
                    query: {
                        ...(workspaceId ? { workspaceId } : {}),
                        ...(spaceId && spaceId !== "global" && !workspaceId ? { spaceId } : {})
                    }
                })
            );
        }
    });

    // Read URL search params
    const search = useSearch({ strict: false }) as {
        pathFrom?: string;
        pathTo?: string;
        focus?: string;
        tagTypeId?: string;
        zoomLevel?: string;
        island?: string;
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

    // Scoped chunk tags (only chunks present in the graph)
    const scopedChunkTags = useMemo(() => {
        if (!data?.chunkTags || !data?.chunks) return [] as NonNullable<typeof data>["chunkTags"];
        const chunkIds = new Set(data.chunks.map((c: { id: string }) => c.id));
        return data.chunkTags.filter((ct: { chunkId: string }) => chunkIds.has(ct.chunkId));
    }, [data?.chunkTags, data?.chunks]);

    const availableTagTypeIds = useMemo(() => {
        const ids = new Set<string>();
        for (const ct of scopedChunkTags as Array<{ tagTypeId?: string | null }>) {
            if (ct.tagTypeId) ids.add(ct.tagTypeId);
        }
        return ids;
    }, [scopedChunkTags]);

    // Pick the tag type with the best chunk coverage (most chunks tagged)
    const bestTagTypeId = useMemo(() => {
        if (availableTagTypeIds.size === 0) return null;
        const counts = new Map<string, number>();
        const seen = new Map<string, Set<string>>();
        for (const ct of scopedChunkTags as Array<{ chunkId: string; tagTypeId?: string | null }>) {
            if (!ct.tagTypeId) continue;
            const s = seen.get(ct.tagTypeId) ?? new Set();
            s.add(ct.chunkId);
            seen.set(ct.tagTypeId, s);
            counts.set(ct.tagTypeId, s.size);
        }
        let best: string | null = null;
        let bestCount = 0;
        for (const [id, count] of counts) {
            if (count > bestCount) {
                best = id;
                bestCount = count;
            }
        }
        return best;
    }, [scopedChunkTags, availableTagTypeIds]);

    // Grouping tag type — URL param > best coverage > first available
    const [groupingTagTypeId, setGroupingTagTypeId] = useState<string | null>(null);

    useEffect(() => {
        if (search.tagTypeId && availableTagTypeIds.has(search.tagTypeId)) {
            setGroupingTagTypeId(search.tagTypeId);
        } else if (groupingTagTypeId === null) {
            setGroupingTagTypeId(bestTagTypeId);
        }
    }, [search.tagTypeId, availableTagTypeIds, groupingTagTypeId, bestTagTypeId]);

    // Sync to graph state
    useEffect(() => {
        dispatch({ type: "SET_GROUPING_TAG_TYPE", id: groupingTagTypeId });
    }, [groupingTagTypeId, dispatch]);

    // Island computation
    const islandData = useMemo<IslandFormationResult>(() => {
        if (!data?.chunks || !groupingTagTypeId) {
            return { islands: [], bridges: [], chunkToIsland: new Map() };
        }

        const chunkTags = scopedChunkTags as Array<{ chunkId: string; tagTypeId: string | null; tagName: string }>;

        return formIslands({
            chunks: data.chunks,
            connections: data.connections ?? [],
            chunkTags,
            groupingTagTypeId
        });
    }, [data?.chunks, data?.connections, scopedChunkTags, groupingTagTypeId]);

    // Initial focus/island from URL
    const initialFocusChunkId = search.focus ?? null;
    const initialIslandId = search.island ?? null;

    // Compute simplified health scores from chunk metadata (freshness based on createdAt)
    const chunkHealthScores = useMemo(() => {
        const scores = new Map<string, number>();
        if (!data?.chunks) return scores;
        const now = Date.now();
        for (const chunk of data.chunks as Array<{ id: string; createdAt: Date | string }>) {
            const age = (now - new Date(chunk.createdAt).getTime()) / (1000 * 60 * 60 * 24);
            const freshness = Math.max(0, Math.min(100, 100 - (age / 180) * 100));
            scores.set(chunk.id, Math.round(freshness));
        }
        return scores;
    }, [data?.chunks]);

    // Aggregate health scores per island
    const islandHealthScores = useMemo(() => {
        const map = new Map<string, number[]>();
        if (!islandData) return map;
        for (const island of islandData.islands) {
            map.set(
                island.id,
                island.chunkIds.map(id => chunkHealthScores.get(id) ?? 50)
            );
        }
        return map;
    }, [islandData, chunkHealthScores]);

    return {
        data,
        isLoading,
        spaceId,
        workspaceId,
        scopedChunkTags,
        availableTagTypeIds,
        groupingTagTypeId,
        setGroupingTagTypeId,
        islandData,
        initialFocusChunkId,
        initialIslandId,
        chunkHealthScores,
        islandHealthScores
    };
}
