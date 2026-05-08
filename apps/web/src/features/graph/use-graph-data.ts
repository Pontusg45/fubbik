/**
 * Data fetching, scoping, and URL-driven prefilter for the graph view.
 *
 * Owns the graph API query and derives scoped chunk-tags + available tag-type IDs
 * so downstream hooks receive pre-filtered, codebase-aware data.
 */

import { useQuery } from "@tanstack/react-query";
import { useSearch } from "@tanstack/react-router";
import { useEffect, useMemo, useState } from "react";

import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { EMPTY_FILTER, type GraphFilterValues } from "@/features/graph/graph-filter-dialog";
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

    // Read path params from URL search
    const search = useSearch({ strict: false }) as {
        pathFrom?: string;
        pathTo?: string;
        tags?: string;
        types?: string;
        focus?: string;
        depth?: number;
        groupBy?: "tag" | "type" | "codebase" | "none";
        tagTypeId?: string;
        all?: number;
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

    // Pre-filter driven by URL params (set via GraphFilterDialog on entry)
    const prefilter = useMemo<GraphFilterValues>(() => {
        if (!search.tags && !search.types && !search.focus && !search.groupBy) return EMPTY_FILTER;
        return {
            tags: search.tags ? search.tags.split(",").filter(Boolean) : [],
            types: search.types ? search.types.split(",").filter(Boolean) : [],
            focusChunkId: search.focus ?? null,
            depth: typeof search.depth === "number" && search.depth >= 1 && search.depth <= 3 ? search.depth : 2,
            groupBy: search.groupBy ?? "tag",
            tagTypeId: search.tagTypeId ?? null,
            subGroupBy: null,
        };
    }, [search.tags, search.types, search.focus, search.depth, search.groupBy, search.tagTypeId]);

    const hasAnyFilterParams = !!(search.tags || search.types || search.focus || search.groupBy || search.all);
    const [filterDialogOpen, setFilterDialogOpen] = useState(!hasAnyFilterParams);

    // chunkTags from the API are not codebase-filtered — filter to only chunks in the graph
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

    // Apply groupBy from prefilter once graph data is available
    useEffect(() => {
        if (!data?.tagTypes) return;
        if (prefilter.groupBy === "tag" && prefilter.tagTypeId) {
            dispatch({ type: "SET_ACTIVE_TAG_TYPE_IDS", ids: new Set([prefilter.tagTypeId]) });
        } else if (prefilter.groupBy === "tag") {
            const ids = new Set(data.tagTypes.map(tt => tt.id));
            if (ids.size > 0) dispatch({ type: "SET_ACTIVE_TAG_TYPE_IDS", ids });
        } else {
            dispatch({ type: "SET_ACTIVE_TAG_TYPE_IDS", ids: new Set() });
        }
    }, [prefilter.groupBy, prefilter.tagTypeId, data?.tagTypes, dispatch]);

    return {
        data,
        isLoading,
        codebaseId,
        workspaceId,
        prefilter,
        filterDialogOpen,
        setFilterDialogOpen,
        scopedChunkTags,
        availableTagTypeIds,
    };
}
