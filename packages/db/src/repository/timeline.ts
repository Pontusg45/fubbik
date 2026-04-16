import { Effect } from "effect";
import { sql } from "drizzle-orm";

import { DatabaseError } from "../errors";
import { db } from "../index";

export interface TimelineEvent {
    chunkId: string;
    chunkTitle: string;
    chunkType: string;
    kind: "created" | "updated";
    at: string;
    version: number | null;
}

export interface TimelineParams {
    userId: string;
    from: Date;
    codebaseId?: string;
    tag?: string;
}

export function fetchTimeline(params: TimelineParams) {
    return Effect.tryPromise({
        try: async (): Promise<TimelineEvent[]> => {
            const codebaseFilter = params.codebaseId
                ? sql`AND c.id IN (SELECT chunk_id FROM chunk_codebase WHERE codebase_id = ${params.codebaseId})`
                : sql``;
            const tagFilter = params.tag
                ? sql`AND c.id IN (
                    SELECT ct.chunk_id FROM chunk_tag ct
                    INNER JOIN tag t ON t.id = ct.tag_id
                    WHERE t.name = ${params.tag}
                )`
                : sql``;

            const result = await db.execute(sql`
                SELECT chunk_id, chunk_title, chunk_type, kind, at, version
                FROM (
                    SELECT
                        c.id AS chunk_id,
                        c.title AS chunk_title,
                        c.type AS chunk_type,
                        'created'::text AS kind,
                        c.created_at AS at,
                        NULL::integer AS version
                    FROM chunk c
                    WHERE c.user_id = ${params.userId}
                      AND c.archived_at IS NULL
                      AND c.created_at >= ${params.from.toISOString()}
                      ${codebaseFilter}
                      ${tagFilter}

                    UNION ALL

                    SELECT
                        cv.chunk_id AS chunk_id,
                        c.title AS chunk_title,
                        c.type AS chunk_type,
                        'updated'::text AS kind,
                        cv.created_at AS at,
                        cv.version AS version
                    FROM chunk_version cv
                    INNER JOIN chunk c ON c.id = cv.chunk_id
                    WHERE c.user_id = ${params.userId}
                      AND c.archived_at IS NULL
                      AND cv.created_at >= ${params.from.toISOString()}
                      ${codebaseFilter}
                      ${tagFilter}
                ) combined
                ORDER BY at DESC
                LIMIT 500
            `);

            const rows = result.rows as Array<{
                chunk_id: string;
                chunk_title: string;
                chunk_type: string;
                kind: "created" | "updated";
                at: string | Date;
                version: number | null;
            }>;

            return rows.map(r => ({
                chunkId: r.chunk_id,
                chunkTitle: r.chunk_title,
                chunkType: r.chunk_type,
                kind: r.kind,
                at: typeof r.at === "string" ? r.at : r.at.toISOString(),
                version: r.version
            }));
        },
        catch: cause => new DatabaseError({ cause })
    });
}
