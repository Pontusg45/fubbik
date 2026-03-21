import { and, desc, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import {
    implementationSession,
    sessionAssumption,
    sessionChunkRef,
    sessionRequirementRef
} from "../schema/implementation-session";
import { requirement } from "../schema/requirement";

export function createSession(params: { id: string; title: string; userId: string; codebaseId?: string; planId?: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(implementationSession).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getSessionById(id: string, userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(implementationSession.id, id)];
            if (userId) conditions.push(eq(implementationSession.userId, userId));
            const [found] = await db
                .select()
                .from(implementationSession)
                .where(and(...conditions));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function listSessions(params: {
    userId: string;
    status?: string;
    codebaseId?: string;
    limit: number;
    offset: number;
}) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(implementationSession.userId, params.userId)];
            if (params.status) conditions.push(eq(implementationSession.status, params.status));
            if (params.codebaseId) conditions.push(eq(implementationSession.codebaseId, params.codebaseId));

            const sessions = await db
                .select()
                .from(implementationSession)
                .where(and(...conditions))
                .orderBy(desc(implementationSession.createdAt))
                .limit(params.limit)
                .offset(params.offset);

            const total = await db
                .select({ count: sql<number>`count(*)` })
                .from(implementationSession)
                .where(and(...conditions));

            return { sessions, total: Number(total[0]?.count ?? 0) };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateSession(
    id: string,
    userId: string,
    params: {
        status?: string;
        prUrl?: string;
        reviewBrief?: string;
        completedAt?: Date | null;
        reviewedAt?: Date | null;
    }
) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = {};
            if (params.status !== undefined) setClause.status = params.status;
            if (params.prUrl !== undefined) setClause.prUrl = params.prUrl;
            if (params.reviewBrief !== undefined) setClause.reviewBrief = params.reviewBrief;
            if (params.completedAt !== undefined) setClause.completedAt = params.completedAt;
            if (params.reviewedAt !== undefined) setClause.reviewedAt = params.reviewedAt;

            if (Object.keys(setClause).length === 0) {
                const [found] = await db
                    .select()
                    .from(implementationSession)
                    .where(and(eq(implementationSession.id, id), eq(implementationSession.userId, userId)));
                return found ?? null;
            }

            const [updated] = await db
                .update(implementationSession)
                .set(setClause)
                .where(and(eq(implementationSession.id, id), eq(implementationSession.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function addChunkRef(sessionId: string, chunkId: string, reason: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .insert(sessionChunkRef)
                .values({ sessionId, chunkId, reason })
                .onConflictDoNothing()
                .returning(),
        catch: cause => new DatabaseError({ cause })
    });
}

export function addAssumption(params: { id: string; sessionId: string; description: string }) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(sessionAssumption).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function resolveAssumption(id: string, params: { resolved: boolean; resolution?: string }) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = { resolved: params.resolved };
            if (params.resolution !== undefined) setClause.resolution = params.resolution;

            const [updated] = await db
                .update(sessionAssumption)
                .set(setClause)
                .where(eq(sessionAssumption.id, id))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function addRequirementRef(sessionId: string, requirementId: string, stepsAddressed: number[] = []) {
    return Effect.tryPromise({
        try: () =>
            db
                .insert(sessionRequirementRef)
                .values({ sessionId, requirementId, stepsAddressed })
                .onConflictDoNothing()
                .returning(),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getSessionDetail(id: string) {
    return Effect.tryPromise({
        try: async () => {
            const [session] = await db
                .select()
                .from(implementationSession)
                .where(eq(implementationSession.id, id));

            const chunkRefs = await db
                .select({
                    sessionId: sessionChunkRef.sessionId,
                    chunkId: sessionChunkRef.chunkId,
                    reason: sessionChunkRef.reason,
                    chunkTitle: chunk.title
                })
                .from(sessionChunkRef)
                .innerJoin(chunk, eq(sessionChunkRef.chunkId, chunk.id))
                .where(eq(sessionChunkRef.sessionId, id));

            const assumptions = await db
                .select()
                .from(sessionAssumption)
                .where(eq(sessionAssumption.sessionId, id));

            const requirementRefs = await db
                .select({
                    sessionId: sessionRequirementRef.sessionId,
                    requirementId: sessionRequirementRef.requirementId,
                    stepsAddressed: sessionRequirementRef.stepsAddressed,
                    requirementTitle: requirement.title,
                    requirementStatus: requirement.status,
                    steps: requirement.steps
                })
                .from(sessionRequirementRef)
                .innerJoin(requirement, eq(sessionRequirementRef.requirementId, requirement.id))
                .where(eq(sessionRequirementRef.sessionId, id));

            return {
                session: session ?? null,
                chunkRefs,
                assumptions,
                requirementRefs: requirementRefs.map(ref => ({
                    ...ref,
                    totalSteps: ref.steps?.length ?? 0
                }))
            };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getSessionsForRequirement(requirementId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: implementationSession.id,
                    title: implementationSession.title,
                    status: implementationSession.status,
                    createdAt: implementationSession.createdAt
                })
                .from(sessionRequirementRef)
                .innerJoin(
                    implementationSession,
                    eq(sessionRequirementRef.sessionId, implementationSession.id)
                )
                .where(eq(sessionRequirementRef.requirementId, requirementId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getUnresolvedAssumptionsSummary(userId: string) {
    return Effect.tryPromise({
        try: () =>
            db.execute(
                sql`
                    SELECT sa.description, count(*) as frequency, array_agg(sa.session_id) as session_ids
                    FROM session_assumption sa
                    INNER JOIN implementation_session s ON s.id = sa.session_id
                    WHERE sa.resolved = false AND s.user_id = ${userId}
                    GROUP BY sa.description
                    ORDER BY count(*) DESC
                    LIMIT 20
                `
            ),
        catch: cause => new DatabaseError({ cause })
    });
}
