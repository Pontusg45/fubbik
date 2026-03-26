import { and, eq, isNull, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunk } from "../schema/chunk";
import { chunkCodebase } from "../schema/codebase";
import { implementationSession, sessionRequirementRef } from "../schema/implementation-session";
import { plan, planStep } from "../schema/plan";
import { requirement, requirementChunk } from "../schema/requirement";

export function getChunkCoverage(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(chunk.userId, userId), isNull(chunk.archivedAt)];

            let chunkQuery;
            if (codebaseId) {
                chunkQuery = db
                    .select({
                        id: chunk.id,
                        title: chunk.title,
                        requirementCount: sql<number>`count(${requirementChunk.requirementId})`
                    })
                    .from(chunk)
                    .innerJoin(chunkCodebase, eq(chunkCodebase.chunkId, chunk.id))
                    .leftJoin(requirementChunk, eq(requirementChunk.chunkId, chunk.id))
                    .where(and(...conditions, eq(chunkCodebase.codebaseId, codebaseId)))
                    .groupBy(chunk.id, chunk.title);
            } else {
                chunkQuery = db
                    .select({
                        id: chunk.id,
                        title: chunk.title,
                        requirementCount: sql<number>`count(${requirementChunk.requirementId})`
                    })
                    .from(chunk)
                    .leftJoin(requirementChunk, eq(requirementChunk.chunkId, chunk.id))
                    .where(and(...conditions))
                    .groupBy(chunk.id, chunk.title);
            }

            return chunkQuery;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getChunkCoverageMatrix(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            if (codebaseId) {
                return db
                    .select({
                        chunkId: requirementChunk.chunkId,
                        chunkTitle: chunk.title,
                        requirementId: requirementChunk.requirementId,
                        requirementTitle: requirement.title,
                        requirementStatus: requirement.status
                    })
                    .from(requirementChunk)
                    .innerJoin(chunk, eq(requirementChunk.chunkId, chunk.id))
                    .innerJoin(requirement, eq(requirementChunk.requirementId, requirement.id))
                    .innerJoin(chunkCodebase, eq(chunkCodebase.chunkId, chunk.id))
                    .where(and(
                        eq(chunk.userId, userId),
                        isNull(chunk.archivedAt),
                        eq(chunkCodebase.codebaseId, codebaseId)
                    ));
            } else {
                return db
                    .select({
                        chunkId: requirementChunk.chunkId,
                        chunkTitle: chunk.title,
                        requirementId: requirementChunk.requirementId,
                        requirementTitle: requirement.title,
                        requirementStatus: requirement.status
                    })
                    .from(requirementChunk)
                    .innerJoin(chunk, eq(requirementChunk.chunkId, chunk.id))
                    .innerJoin(requirement, eq(requirementChunk.requirementId, requirement.id))
                    .where(and(
                        eq(chunk.userId, userId),
                        isNull(chunk.archivedAt)
                    ));
            }
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTraceabilityMatrix(userId: string, codebaseId?: string) {
    return Effect.tryPromise({
        try: async () => {
            // 1. Get all requirements
            const conditions = [eq(requirement.userId, userId)];
            if (codebaseId) {
                conditions.push(eq(requirement.codebaseId, codebaseId));
            }
            const requirements = await db
                .select({
                    id: requirement.id,
                    title: requirement.title,
                    status: requirement.status,
                    priority: requirement.priority
                })
                .from(requirement)
                .where(and(...conditions));

            // 2. Get plan steps linked to requirements
            const planStepRows = await db
                .select({
                    requirementId: planStep.requirementId,
                    stepId: planStep.id,
                    stepDescription: planStep.description,
                    stepStatus: planStep.status,
                    planId: plan.id,
                    planTitle: plan.title,
                    planStatus: plan.status
                })
                .from(planStep)
                .innerJoin(plan, eq(planStep.planId, plan.id))
                .where(and(
                    eq(plan.userId, userId),
                    sql`${planStep.requirementId} IS NOT NULL`
                ));

            // 3. Get sessions that addressed requirements
            const sessionRows = await db
                .select({
                    requirementId: sessionRequirementRef.requirementId,
                    sessionId: implementationSession.id,
                    sessionTitle: implementationSession.title,
                    sessionStatus: implementationSession.status
                })
                .from(sessionRequirementRef)
                .innerJoin(
                    implementationSession,
                    eq(sessionRequirementRef.sessionId, implementationSession.id)
                )
                .where(eq(implementationSession.userId, userId));

            // 4. Assemble traceability per requirement
            const planStepMap = new Map<string, Array<{
                stepId: string;
                stepDescription: string;
                stepStatus: string;
                planId: string;
                planTitle: string;
                planStatus: string;
            }>>();
            for (const row of planStepRows) {
                if (!row.requirementId) continue;
                if (!planStepMap.has(row.requirementId)) {
                    planStepMap.set(row.requirementId, []);
                }
                planStepMap.get(row.requirementId)!.push({
                    stepId: row.stepId,
                    stepDescription: row.stepDescription,
                    stepStatus: row.stepStatus,
                    planId: row.planId,
                    planTitle: row.planTitle,
                    planStatus: row.planStatus
                });
            }

            const sessionMap = new Map<string, Array<{
                sessionId: string;
                sessionTitle: string;
                sessionStatus: string;
            }>>();
            for (const row of sessionRows) {
                if (!sessionMap.has(row.requirementId)) {
                    sessionMap.set(row.requirementId, []);
                }
                sessionMap.get(row.requirementId)!.push({
                    sessionId: row.sessionId,
                    sessionTitle: row.sessionTitle,
                    sessionStatus: row.sessionStatus
                });
            }

            return requirements.map(req => ({
                ...req,
                planSteps: planStepMap.get(req.id) ?? [],
                sessions: sessionMap.get(req.id) ?? []
            }));
        },
        catch: cause => new DatabaseError({ cause })
    });
}
