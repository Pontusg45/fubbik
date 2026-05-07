import {
    getGroupedCounts as getGroupedCountsRepo,
    getChunksInGroup as getChunksInGroupRepo,
} from "@fubbik/db/repository";
import type { GroupedCountsParams, GroupChunksParams } from "@fubbik/db/repository";
import { Effect } from "effect";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type GroupBy = GroupedCountsParams["groupBy"];

/**
 * Parse a raw groupBy string from the query into a typed enum + optional
 * tagTypeId. Accepts "type", "status", "origin", "freshness", plain "tagtype",
 * or "tagtype:<id>".
 */
function parseGroupBy(raw: string): { groupBy: GroupBy; tagTypeId?: string } {
    switch (raw) {
        case "type":
        case "status":
        case "origin":
        case "freshness":
            return { groupBy: raw };
        case "tagtype":
            return { groupBy: "tagtype" };
        default:
            if (raw.startsWith("tagtype:")) {
                return { groupBy: "tagtype", tagTypeId: raw.slice("tagtype:".length) };
            }
            return { groupBy: "type" };
    }
}

function parseTags(raw?: string): string[] | undefined {
    if (!raw) return undefined;
    const parsed = raw.split(",").map(s => s.trim()).filter(Boolean);
    return parsed.length > 0 ? parsed : undefined;
}

// ---------------------------------------------------------------------------
// Service functions
// ---------------------------------------------------------------------------

export function listGroupedCounts(
    userId: string,
    query: {
        groupBy: string;
        tagTypeId?: string;
        codebaseId?: string;
        workspaceId?: string;
        global?: string;
        type?: string;
        origin?: string;
        reviewStatus?: string;
        tags?: string;
        tagMode?: "any" | "all";
        search?: string;
    }
) {
    const { groupBy, tagTypeId } = parseGroupBy(query.groupBy);
    const tags = parseTags(query.tags);
    const globalOnly = query.global === "true";

    const params: GroupedCountsParams = {
        groupBy,
        tagTypeId: query.tagTypeId ?? tagTypeId,
        userId,
        type: query.type,
        origin: query.origin,
        reviewStatus: query.reviewStatus,
        codebaseId: query.codebaseId,
        workspaceId: query.workspaceId,
        globalOnly,
        tags,
        tagMode: query.tagMode,
    };

    return getGroupedCountsRepo(params).pipe(
        Effect.map(groups => ({
            groups,
            totalGroups: groups.length,
        }))
    );
}

export function listGroupChunks(
    userId: string,
    groupName: string,
    query: {
        groupBy: string;
        tagTypeId?: string;
        codebaseId?: string;
        workspaceId?: string;
        global?: string;
        type?: string;
        origin?: string;
        reviewStatus?: string;
        tags?: string;
        tagMode?: "any" | "all";
        search?: string;
        sort?: string;
        limit?: string;
        offset?: string;
    }
) {
    const { groupBy, tagTypeId } = parseGroupBy(query.groupBy);
    const tags = parseTags(query.tags);
    const globalOnly = query.global === "true";
    const limit = Math.min(Number(query.limit ?? 50), 100);
    const offset = Number(query.offset ?? 0);

    const params: GroupChunksParams = {
        groupBy,
        groupName,
        tagTypeId: query.tagTypeId ?? tagTypeId,
        userId,
        type: query.type,
        origin: query.origin,
        reviewStatus: query.reviewStatus,
        codebaseId: query.codebaseId,
        workspaceId: query.workspaceId,
        globalOnly,
        tags,
        tagMode: query.tagMode,
        limit,
        offset,
    };

    return getChunksInGroupRepo(params).pipe(
        Effect.map(result => ({
            chunks: result.chunks,
            total: result.total,
            limit,
            offset,
        }))
    );
}
