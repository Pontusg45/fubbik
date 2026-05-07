/**
 * Cluster aggregation for large graphs (500+ nodes).
 *
 * When the total node count exceeds a threshold, each group collapses into a
 * single "cluster node". Expanding a cluster reveals its members. This keeps
 * React Flow performant on graphs that would otherwise render hundreds of DOM
 * nodes simultaneously.
 */

import type { GroupStrategyResult } from "./group-strategies";

export interface ClusterNode {
    id: string;
    groupName: string;
    count: number;
    color?: string;
    expanded: boolean;
}

/**
 * Decide whether clustering is needed and build one ClusterNode per group.
 */
export function buildClusterNodes(
    groupResult: GroupStrategyResult,
    expandedGroups: Set<string>,
    threshold: number = 500
): { clusters: ClusterNode[]; shouldCluster: boolean } {
    let totalNodes = 0;
    for (const chunkIds of groupResult.groups.values()) {
        totalNodes += chunkIds.length;
    }

    if (totalNodes < threshold) {
        return { clusters: [], shouldCluster: false };
    }

    const clusters: ClusterNode[] = [];
    for (const [groupName, chunkIds] of groupResult.groups) {
        clusters.push({
            id: `cluster-${groupName}`,
            groupName,
            count: chunkIds.length,
            color: groupResult.colorFor(groupName),
            expanded: expandedGroups.has(groupName),
        });
    }

    return { clusters, shouldCluster: true };
}

/**
 * Return the set of chunk IDs that should be visible given the current
 * clustering and expansion state.
 */
export function getVisibleChunkIds(
    groupResult: GroupStrategyResult,
    expandedGroups: Set<string>,
    shouldCluster: boolean
): Set<string> {
    const visible = new Set<string>();

    if (!shouldCluster) {
        // No clustering — all chunks visible
        for (const chunkIds of groupResult.groups.values()) {
            for (const id of chunkIds) {
                visible.add(id);
            }
        }
        return visible;
    }

    // Only chunks from expanded groups are visible
    for (const [groupName, chunkIds] of groupResult.groups) {
        if (expandedGroups.has(groupName)) {
            for (const id of chunkIds) {
                visible.add(id);
            }
        }
    }

    return visible;
}
