/**
 * Data fetching, scoping, and island computation for the graph view.
 *
 * Owns the graph API query, derives scoped chunk-tags + available tag-type IDs,
 * and computes island formation from the selected grouping tag type.
 */

import { useQuery } from "@tanstack/react-query";
import { useSearch } from "@tanstack/react-router";
import { useEffect, useMemo, useState } from "react";

import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { formIslands, type IslandFormationResult } from "@/features/graph/island-formation";
import type { GraphAction } from "@/features/graph/use-graph-state";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export type GraphData = NonNullable<ReturnType<typeof useGraphData>["data"]>;

export function useGraphData(dispatch: React.Dispatch<GraphAction>) {
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

    // Grouping tag type — auto-select first available, or from URL
    const [groupingTagTypeId, setGroupingTagTypeId] = useState<string | null>(null);

    useEffect(() => {
        if (search.tagTypeId && availableTagTypeIds.has(search.tagTypeId)) {
            setGroupingTagTypeId(search.tagTypeId);
        } else if (groupingTagTypeId === null && availableTagTypeIds.size > 0) {
            const first = [...availableTagTypeIds][0]!;
            setGroupingTagTypeId(first);
        }
    }, [search.tagTypeId, availableTagTypeIds, groupingTagTypeId]);

    // Sync to graph state
    useEffect(() => {
        dispatch({ type: "SET_GROUPING_TAG_TYPE", id: groupingTagTypeId });
    }, [groupingTagTypeId, dispatch]);

    // Island computation
    const islandData = useMemo<IslandFormationResult>(() => {
        if (!data?.chunks || !groupingTagTypeId) {
            return { islands: [], bridges: [], chunkToIsland: new Map() };
        }

        const chunkTags = (scopedChunkTags as Array<{ chunkId: string; tagTypeId: string | null; tagName: string }>);

        return formIslands({
            chunks: data.chunks,
            connections: data.connections ?? [],
            chunkTags,
            groupingTagTypeId,
        });
    }, [data?.chunks, data?.connections, scopedChunkTags, groupingTagTypeId]);

    // Initial focus/island from URL
    const initialFocusChunkId = search.focus ?? null;
    const initialIslandId = search.island ?? null;

    return {
        data,
        isLoading,
        codebaseId,
        workspaceId,
        scopedChunkTags,
        availableTagTypeIds,
        groupingTagTypeId,
        setGroupingTagTypeId,
        islandData,
        initialFocusChunkId,
        initialIslandId,
    };
}
