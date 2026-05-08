/**
 * Grouping resolution for the graph view.
 *
 * Resolves the active grouping strategy from prefilter + tag-type toggles,
 * computes group membership, cluster aggregation, and chunk-to-tag-group lookups.
 */

import { useMemo } from "react";

import { buildClusterNodes, getVisibleChunkIds as getClusterVisibleChunkIds } from "@/features/graph/cluster-strategy";
import type { GraphFilterValues } from "@/features/graph/graph-filter-dialog";
import { GROUP_STRATEGIES, type GroupBy } from "@/features/graph/group-strategies";
import { isDev } from "@/features/graph/graph-timings";
import type { GraphData } from "@/features/graph/use-graph-data";

interface UseGraphGroupingParams {
    data: GraphData | undefined;
    prefilter: GraphFilterValues;
    activeTagTypeIds: Set<string>;
    availableTagTypeIds: Set<string>;
    effectiveTagTypeIdsOverride?: Set<string>;
    expandedClusters: Set<string>;
    scopedChunkTags: GraphData["chunkTags"];
    TYPE_COLORS: Record<string, { bg: string; border: string }>;
}

export function useGraphGrouping({
    data,
    prefilter,
    activeTagTypeIds,
    availableTagTypeIds,
    expandedClusters,
    scopedChunkTags,
    TYPE_COLORS,
}: UseGraphGroupingParams) {
    // Resolve active grouping strategy from prefilter + existing tag-type toggles.
    // prefilter.groupBy explicitly wins when set to something other than "tag" (its default).
    // "tag" falls back to activeTagTypeIds (the in-graph toggle).
    // When groupBy is "tag" but activeTagTypeIds hasn't been set yet (first render),
    // use availableTagTypeIds so groups render immediately without a useEffect gap.
    const effectiveTagTypeIds = useMemo(() => {
        let result: Set<string>;
        let source: string;
        if (prefilter.tagTypeId) {
            result = new Set([prefilter.tagTypeId]);
            source = `prefilter.tagTypeId=${prefilter.tagTypeId}`;
        } else if (activeTagTypeIds.size > 0) {
            result = activeTagTypeIds;
            source = `activeTagTypeIds(${activeTagTypeIds.size})`;
        } else if (prefilter.groupBy === "tag" && availableTagTypeIds.size > 0) {
            result = availableTagTypeIds;
            source = `availableTagTypeIds(${availableTagTypeIds.size})`;
        } else {
            result = activeTagTypeIds;
            source = "fallback-empty";
        }
        if (isDev) console.debug("[graph] effectiveTagTypeIds:", source, [...result]);
        return result;
    }, [activeTagTypeIds, prefilter.groupBy, prefilter.tagTypeId, availableTagTypeIds]);

    const groupingMode: Exclude<GroupBy, "none"> | null = useMemo(() => {
        if (prefilter.groupBy === "type") return "type";
        if (prefilter.groupBy === "codebase") return "codebase";
        if (prefilter.groupBy === "none") return null;
        if (effectiveTagTypeIds.size > 0) return "tag";
        return null;
    }, [prefilter.groupBy, effectiveTagTypeIds]);

    const groupResult = useMemo(() => {
        if (!groupingMode || !data) {
            if (isDev) console.debug("[graph] groupResult: null (mode=%s, hasData=%s)", groupingMode, !!data);
            return null;
        }
        const typeColorMap: Record<string, string> = {};
        for (const [name, palette] of Object.entries(TYPE_COLORS)) typeColorMap[name] = palette.border;
        const result = GROUP_STRATEGIES[groupingMode].build({
            chunks: data.chunks,
            chunkTags: scopedChunkTags,
            activeTagTypeIds: effectiveTagTypeIds,
            chunkCodebases: data.chunkCodebases,
            typeColorMap
        });
        if (isDev) console.debug("[graph] groupResult:", groupingMode, result ? `${result.groups.size} groups` : "null");
        return result;
    }, [groupingMode, data, effectiveTagTypeIds, TYPE_COLORS, scopedChunkTags]);

    // Keep `tagGroups` variable name — downstream pipeline references it by this name.
    const tagGroups = groupResult?.groups ?? null;

    // --- Cluster aggregation for large graphs ---
    const { clusters, shouldCluster } = useMemo(() => {
        if (!groupResult) return { clusters: [] as ReturnType<typeof buildClusterNodes>["clusters"], shouldCluster: false };
        return buildClusterNodes(groupResult, expandedClusters);
    }, [groupResult, expandedClusters]);

    const clusterVisibleChunkIds = useMemo(() => {
        if (!groupResult) return null;
        return getClusterVisibleChunkIds(groupResult, expandedClusters, shouldCluster);
    }, [groupResult, expandedClusters, shouldCluster]);

    // Build chunk-to-tag-group lookup for edge opacity
    const chunkTagGroupMap = useMemo(() => {
        if (!tagGroups) return null;
        const map = new Map<string, Set<string>>();
        for (const [tagName, chunkIds] of tagGroups) {
            for (const cid of chunkIds) {
                if (!map.has(cid)) map.set(cid, new Set());
                map.get(cid)!.add(tagName);
            }
        }
        return map;
    }, [tagGroups]);

    return {
        effectiveTagTypeIds,
        groupingMode,
        groupResult,
        tagGroups,
        clusters,
        shouldCluster,
        clusterVisibleChunkIds,
        chunkTagGroupMap,
    };
}
