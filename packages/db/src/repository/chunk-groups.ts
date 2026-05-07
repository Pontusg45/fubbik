import { Effect } from "effect";
import { and, countDistinct, eq, inArray, isNull, sql } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { chunk } from "../schema/chunk";
import { chunkCodebase } from "../schema/codebase";
import { tag, chunkTag } from "../schema/tag";
import { workspaceCodebase } from "../schema/workspace";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface GroupCount {
    groupName: string;
    count: number;
}

export interface CompoundGroupCount {
    groupName: string;
    count: number;
    subGroups: GroupCount[];
}

export interface CompoundGroupedCountsParams extends GroupedCountsParams {
    subGroupBy: "type" | "status" | "origin" | "freshness" | "tagtype";
    /** Required when subGroupBy is "tagtype" */
    subTagTypeId?: string;
}

export interface GroupedCountsParams {
    groupBy: "type" | "status" | "origin" | "freshness" | "tagtype";
    /** Required when groupBy is "tagtype" */
    tagTypeId?: string;

    // --- shared filters (mirrors listChunks) ---
    userId?: string;
    type?: string;
    origin?: string;
    reviewStatus?: string;
    codebaseId?: string;
    workspaceId?: string;
    globalOnly?: boolean;
    tags?: string[];
    tagMode?: "any" | "all";
}

export interface GroupChunksParams {
    groupBy: "type" | "status" | "origin" | "freshness" | "tagtype";
    groupName: string;
    /** Required when groupBy is "tagtype" */
    tagTypeId?: string;

    // --- shared filters ---
    userId?: string;
    type?: string;
    origin?: string;
    reviewStatus?: string;
    codebaseId?: string;
    workspaceId?: string;
    globalOnly?: boolean;
    tags?: string[];
    tagMode?: "any" | "all";

    limit: number;
    offset: number;
}

// ---------------------------------------------------------------------------
// Shared condition builder (mirrors listChunks patterns)
// ---------------------------------------------------------------------------

function buildBaseConditions(params: {
    userId?: string;
    type?: string;
    origin?: string;
    reviewStatus?: string;
    codebaseId?: string;
    workspaceId?: string;
    globalOnly?: boolean;
    tags?: string[];
    tagMode?: "any" | "all";
}) {
    const conditions = params.userId ? [eq(chunk.userId, params.userId)] : [];

    // Always exclude archived
    conditions.push(isNull(chunk.archivedAt));

    if (params.type) {
        conditions.push(eq(chunk.type, params.type));
    }
    if (params.origin) {
        conditions.push(eq(chunk.origin, params.origin));
    }
    if (params.reviewStatus) {
        conditions.push(eq(chunk.reviewStatus, params.reviewStatus));
    }

    // Tag filters
    if (params.tags && params.tags.length > 0) {
        if (params.tagMode === "all") {
            const tagSubquery = db
                .select({ chunkId: chunkTag.chunkId })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(inArray(tag.name, params.tags))
                .groupBy(chunkTag.chunkId)
                .having(sql`COUNT(DISTINCT ${tag.name}) = ${params.tags.length}`);
            conditions.push(sql`${chunk.id} IN (${tagSubquery})`);
        } else {
            const tagSubquery = db
                .select({ chunkId: chunkTag.chunkId })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(inArray(tag.name, params.tags));
            conditions.push(sql`${chunk.id} IN (${tagSubquery})`);
        }
    }

    // Workspace / codebase scoping
    if (params.workspaceId) {
        const inWorkspace = db
            .select({ codebaseId: workspaceCodebase.codebaseId })
            .from(workspaceCodebase)
            .where(eq(workspaceCodebase.workspaceId, params.workspaceId));
        const inCodebases = db
            .select({ chunkId: chunkCodebase.chunkId })
            .from(chunkCodebase)
            .where(sql`${chunkCodebase.codebaseId} IN (${inWorkspace})`);
        const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
        conditions.push(
            sql`(${chunk.id} IN (${inCodebases}) OR ${chunk.id} NOT IN (${inAnyCodebase}))`
        );
    } else if (params.codebaseId) {
        const inCodebase = db
            .select({ chunkId: chunkCodebase.chunkId })
            .from(chunkCodebase)
            .where(eq(chunkCodebase.codebaseId, params.codebaseId));
        const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
        conditions.push(
            sql`(${chunk.id} IN (${inCodebase}) OR ${chunk.id} NOT IN (${inAnyCodebase}))`
        );
    }

    if (params.globalOnly) {
        const inAnyCodebase = db.select({ chunkId: chunkCodebase.chunkId }).from(chunkCodebase);
        conditions.push(sql`${chunk.id} NOT IN (${inAnyCodebase})`);
    }

    return conditions;
}

// ---------------------------------------------------------------------------
// Freshness CASE expression
// ---------------------------------------------------------------------------

const freshnessBucket = sql<string>`
    CASE
        WHEN ${chunk.updatedAt} >= NOW() - INTERVAL '7 days' THEN 'This week'
        WHEN ${chunk.updatedAt} >= NOW() - INTERVAL '30 days' THEN 'This month'
        WHEN ${chunk.updatedAt} >= NOW() - INTERVAL '90 days' THEN 'Last 3 months'
        ELSE 'Older'
    END
`;

// ---------------------------------------------------------------------------
// getGroupedCounts
// ---------------------------------------------------------------------------

export function getGroupedCounts(params: GroupedCountsParams) {
    return dbEffect(async () => {
        const conditions = buildBaseConditions(params);

        if (params.groupBy === "tagtype") {
            // JOIN chunk_tag + tag, WHERE tag.tagTypeId = param, GROUP BY tag.name
            const tagTypeConditions = params.tagTypeId
                ? [eq(tag.tagTypeId, params.tagTypeId)]
                : [];

            const rows = await db
                .select({
                    groupName: tag.name,
                    count: countDistinct(chunk.id),
                })
                .from(chunk)
                .innerJoin(chunkTag, eq(chunkTag.chunkId, chunk.id))
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(and(...conditions, ...tagTypeConditions))
                .groupBy(tag.name);

            return rows.map(r => ({
                groupName: r.groupName,
                count: Number(r.count),
            }));
        }

        // Determine the GROUP BY expression
        const groupExpr = (() => {
            switch (params.groupBy) {
                case "type":
                    return chunk.type;
                case "status":
                    return sql<string>`COALESCE(${chunk.reviewStatus}, 'draft')`;
                case "origin":
                    return sql<string>`COALESCE(${chunk.origin}, 'human')`;
                case "freshness":
                    return freshnessBucket;
            }
        })();

        const rows = await db
            .select({
                groupName: groupExpr,
                count: sql<number>`count(*)`,
            })
            .from(chunk)
            .where(and(...conditions))
            .groupBy(groupExpr);

        return rows.map(r => ({
            groupName: String(r.groupName),
            count: Number(r.count),
        }));
    });
}

// ---------------------------------------------------------------------------
// Group-specific WHERE clause builder
// ---------------------------------------------------------------------------

function buildGroupCondition(groupBy: GroupChunksParams["groupBy"], groupName: string, tagTypeId?: string) {
    switch (groupBy) {
        case "type":
            return eq(chunk.type, groupName);
        case "status":
            return sql`COALESCE(${chunk.reviewStatus}, 'draft') = ${groupName}`;
        case "origin":
            return sql`COALESCE(${chunk.origin}, 'human') = ${groupName}`;
        case "freshness":
            switch (groupName) {
                case "This week":
                    return sql`${chunk.updatedAt} >= NOW() - INTERVAL '7 days'`;
                case "This month":
                    return sql`${chunk.updatedAt} >= NOW() - INTERVAL '30 days' AND ${chunk.updatedAt} < NOW() - INTERVAL '7 days'`;
                case "Last 3 months":
                    return sql`${chunk.updatedAt} >= NOW() - INTERVAL '90 days' AND ${chunk.updatedAt} < NOW() - INTERVAL '30 days'`;
                case "Older":
                default:
                    return sql`${chunk.updatedAt} < NOW() - INTERVAL '90 days'`;
            }
        case "tagtype": {
            // Chunk must have a tag with the given name under the given tagTypeId
            const tagConditions = tagTypeId
                ? and(eq(tag.name, groupName), eq(tag.tagTypeId, tagTypeId))
                : eq(tag.name, groupName);
            const tagSubquery = db
                .select({ chunkId: chunkTag.chunkId })
                .from(chunkTag)
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(tagConditions!);
            return sql`${chunk.id} IN (${tagSubquery})`;
        }
    }
}

// ---------------------------------------------------------------------------
// getChunksInGroup
// ---------------------------------------------------------------------------

export function getChunksInGroup(params: GroupChunksParams) {
    return dbEffect(async () => {
        const conditions = buildBaseConditions(params);
        const groupCondition = buildGroupCondition(params.groupBy, params.groupName, params.tagTypeId);

        const whereClause = and(...conditions, groupCondition);

        const chunks = await db
            .select()
            .from(chunk)
            .where(whereClause)
            .orderBy(sql`${chunk.updatedAt} DESC`)
            .limit(params.limit)
            .offset(params.offset);

        const total = await db
            .select({ count: sql<number>`count(*)` })
            .from(chunk)
            .where(whereClause);

        return { chunks, total: Number(total[0]?.count ?? 0) };
    });
}

// ---------------------------------------------------------------------------
// getCompoundGroupedCounts — two-level grouping
// ---------------------------------------------------------------------------

export function getCompoundGroupedCounts(params: CompoundGroupedCountsParams) {
    // First get the top-level groups, then for each group run a sub-group query
    // with an extra constraint restricting to chunks in that parent group.
    return getGroupedCounts(params).pipe(
        Effect.flatMap(topGroups =>
            Effect.forEach(
                topGroups,
                (topGroup) => {
                    // Build sub-group params: inherit all base filters, add a constraint
                    // that restricts chunks to those in the parent group
                    const subParams: GroupedCountsParams = {
                        groupBy: params.subGroupBy,
                        tagTypeId: params.subGroupBy === "tagtype" ? params.subTagTypeId : undefined,
                        userId: params.userId,
                        type: params.type,
                        origin: params.origin,
                        reviewStatus: params.reviewStatus,
                        codebaseId: params.codebaseId,
                        workspaceId: params.workspaceId,
                        globalOnly: params.globalOnly,
                        tags: params.tags,
                        tagMode: params.tagMode,
                    };

                    return getSubGroupCounts(subParams, params.groupBy, topGroup.groupName, params.tagTypeId).pipe(
                        Effect.map(subGroups => ({
                            groupName: topGroup.groupName,
                            count: topGroup.count,
                            subGroups,
                        }))
                    );
                },
                { concurrency: 5 }
            )
        )
    );
}

/**
 * Get sub-group counts for chunks that belong to a specific parent group.
 * This is like getGroupedCounts but with an additional WHERE clause constraining
 * chunks to those in the given parent group.
 */
function getSubGroupCounts(
    params: GroupedCountsParams,
    parentGroupBy: GroupedCountsParams["groupBy"],
    parentGroupName: string,
    parentTagTypeId?: string,
) {
    return dbEffect(async () => {
        const conditions = buildBaseConditions(params);
        // Add the parent group constraint
        const parentCondition = buildGroupCondition(parentGroupBy, parentGroupName, parentTagTypeId);
        conditions.push(parentCondition);

        if (params.groupBy === "tagtype") {
            const tagTypeConditions = params.tagTypeId
                ? [eq(tag.tagTypeId, params.tagTypeId)]
                : [];

            const rows = await db
                .select({
                    groupName: tag.name,
                    count: countDistinct(chunk.id),
                })
                .from(chunk)
                .innerJoin(chunkTag, eq(chunkTag.chunkId, chunk.id))
                .innerJoin(tag, eq(chunkTag.tagId, tag.id))
                .where(and(...conditions, ...tagTypeConditions))
                .groupBy(tag.name);

            return rows.map(r => ({
                groupName: r.groupName,
                count: Number(r.count),
            }));
        }

        const groupExpr = (() => {
            switch (params.groupBy) {
                case "type":
                    return chunk.type;
                case "status":
                    return sql<string>`COALESCE(${chunk.reviewStatus}, 'draft')`;
                case "origin":
                    return sql<string>`COALESCE(${chunk.origin}, 'human')`;
                case "freshness":
                    return freshnessBucket;
            }
        })();

        const rows = await db
            .select({
                groupName: groupExpr,
                count: sql<number>`count(*)`,
            })
            .from(chunk)
            .where(and(...conditions))
            .groupBy(groupExpr);

        return rows.map(r => ({
            groupName: String(r.groupName),
            count: Number(r.count),
        }));
    });
}
