/**
 * Pluggable grouping strategies for the graph view.
 *
 * A strategy returns a Map<label, chunkIds[]> + a color for each label. The render
 * pipeline (see graph-view.tsx) stays unchanged — it just asks the active strategy
 * for groups and colors.
 *
 * To add a new grouping mode:
 *   1. Add a new id to GroupBy
 *   2. Write a GroupStrategy and add it to STRATEGIES
 *   3. Update GraphFilterDialog groupBy radio options
 * No changes to rendering, layout worker, or bounding-box code required.
 */

export type GroupBy = "tag" | "type" | "codebase" | "none";

export interface GroupStrategyData {
    chunks: Array<{ id: string; type: string }>;
    chunkTags?: Array<{ chunkId: string; tagTypeId: string | null; tagName: string; tagTypeColor: string | null }>;
    activeTagTypeIds?: Set<string>;
    chunkCodebases?: Array<{ chunkId: string; codebaseId: string; codebaseName: string }>;
    typeColorMap?: Record<string, string>;
}

export interface GroupStrategyResult {
    groups: Map<string, string[]>;
    colorFor: (label: string) => string | undefined;
}

export interface GroupStrategy {
    id: Exclude<GroupBy, "none">;
    build(data: GroupStrategyData): GroupStrategyResult | null;
}

const TAG_STRATEGY: GroupStrategy = {
    id: "tag",
    build({ chunkTags, activeTagTypeIds }) {
        if (!chunkTags || !activeTagTypeIds || activeTagTypeIds.size === 0) return null;
        const groups = new Map<string, string[]>();
        const color = new Map<string, string>();
        for (const ct of chunkTags) {
            if (!ct.tagTypeId || !activeTagTypeIds.has(ct.tagTypeId)) continue;
            if (!groups.has(ct.tagName)) groups.set(ct.tagName, []);
            groups.get(ct.tagName)!.push(ct.chunkId);
            if (ct.tagTypeColor) color.set(ct.tagName, ct.tagTypeColor);
        }
        if (groups.size === 0) return null;
        return { groups, colorFor: label => color.get(label) };
    }
};

const TYPE_STRATEGY: GroupStrategy = {
    id: "type",
    build({ chunks, typeColorMap }) {
        const groups = new Map<string, string[]>();
        for (const c of chunks) {
            if (!groups.has(c.type)) groups.set(c.type, []);
            groups.get(c.type)!.push(c.id);
        }
        if (groups.size === 0) return null;
        return { groups, colorFor: label => typeColorMap?.[label] };
    }
};

const CODEBASE_STRATEGY: GroupStrategy = {
    id: "codebase",
    build({ chunkCodebases }) {
        if (!chunkCodebases || chunkCodebases.length === 0) return null;
        const groups = new Map<string, string[]>();
        for (const cc of chunkCodebases) {
            if (!groups.has(cc.codebaseName)) groups.set(cc.codebaseName, []);
            groups.get(cc.codebaseName)!.push(cc.chunkId);
        }
        if (groups.size === 0) return null;
        return { groups, colorFor: () => undefined };
    }
};

export const GROUP_STRATEGIES: Record<Exclude<GroupBy, "none">, GroupStrategy> = {
    tag: TAG_STRATEGY,
    type: TYPE_STRATEGY,
    codebase: CODEBASE_STRATEGY
};

/** Canonical prefix for group-node IDs in React Flow (was "tag-group-"). */
export const GROUP_NODE_ID_PREFIX = "group-";
export const UNGROUPED_NODE_ID = `${GROUP_NODE_ID_PREFIX}ungrouped`;

export function isGroupNodeId(id: string): boolean {
    // Backwards-compat: accept the old prefix too until any saved state clears.
    return id.startsWith(GROUP_NODE_ID_PREFIX) || id.startsWith("tag-group-");
}
