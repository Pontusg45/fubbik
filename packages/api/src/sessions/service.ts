import {
    createSession as createSessionRepo,
    getSessionById,
    getSessionDetail,
    listSessions as listSessionsRepo,
    updateSession,
    addChunkRef as addChunkRefRepo,
    addAssumption as addAssumptionRepo,
    resolveAssumption as resolveAssumptionRepo,
    addRequirementRef as addRequirementRefRepo,
    updateRequirementStatus,
    listRequirements,
    listChunks
} from "@fubbik/db/repository";
import { Effect } from "effect";
import { NotFoundError } from "../errors";
import { generateReviewBrief } from "./brief-generator";

export function createSession(userId: string, body: { title: string; codebaseId?: string }) {
    return Effect.gen(function* () {
        const id = crypto.randomUUID();
        const session = yield* createSessionRepo({
            id,
            title: body.title,
            userId,
            codebaseId: body.codebaseId
        });

        // Fetch context bundle: conventions, requirements, architecture decisions
        const allChunks = yield* listChunks({
            userId,
            codebaseId: body.codebaseId,
            limit: 100,
            offset: 0
        });

        const conventions = allChunks.chunks.filter(c => c.rationale);
        const architectureDecisions = allChunks.chunks.filter(c => c.type === "document" && c.rationale);

        const reqResult = yield* listRequirements({
            userId,
            codebaseId: body.codebaseId,
            limit: 100,
            offset: 0
        });

        return {
            session,
            context: {
                conventions: conventions.map(c => ({ id: c.id, title: c.title, content: c.content, rationale: c.rationale })),
                requirements: reqResult.requirements.map(r => ({
                    id: r.id,
                    title: r.title,
                    description: r.description,
                    status: r.status,
                    steps: r.steps
                })),
                architectureDecisions: architectureDecisions.map(c => ({
                    id: c.id,
                    title: c.title,
                    content: c.content,
                    rationale: c.rationale
                }))
            }
        };
    });
}

export function getSession(id: string, userId: string) {
    return Effect.gen(function* () {
        const detail = yield* getSessionDetail(id);
        if (!detail.session) {
            return yield* Effect.fail(new NotFoundError({ resource: "session" }));
        }
        if (detail.session.userId !== userId) {
            return yield* Effect.fail(new NotFoundError({ resource: "session" }));
        }

        // Fetch all requirements and conventions for this session's codebase
        const reqResult = yield* listRequirements({
            userId,
            codebaseId: detail.session.codebaseId ?? undefined,
            limit: 100,
            offset: 0
        });

        const allChunks = yield* listChunks({
            userId,
            codebaseId: detail.session.codebaseId ?? undefined,
            limit: 100,
            offset: 0
        });

        const allConventions = allChunks.chunks
            .filter(c => c.rationale)
            .map(c => ({ id: c.id, title: c.title }));

        return {
            ...detail,
            allRequirements: reqResult.requirements.map(r => ({
                id: r.id,
                title: r.title,
                status: r.status,
                steps: r.steps
            })),
            allConventions
        };
    });
}

export function listSessions(
    userId: string,
    query: { status?: string; codebaseId?: string; limit?: number; offset?: number }
) {
    return listSessionsRepo({
        userId,
        status: query.status,
        codebaseId: query.codebaseId,
        limit: query.limit ?? 20,
        offset: query.offset ?? 0
    });
}

export function addChunkRef(sessionId: string, userId: string, chunkId: string, reason: string) {
    return Effect.gen(function* () {
        const session = yield* getSessionById(sessionId, userId);
        if (!session) {
            return yield* Effect.fail(new NotFoundError({ resource: "session" }));
        }
        return yield* addChunkRefRepo(sessionId, chunkId, reason);
    });
}

export function addAssumption(sessionId: string, userId: string, description: string) {
    return Effect.gen(function* () {
        const session = yield* getSessionById(sessionId, userId);
        if (!session) {
            return yield* Effect.fail(new NotFoundError({ resource: "session" }));
        }
        const id = crypto.randomUUID();
        return yield* addAssumptionRepo({ id, sessionId, description });
    });
}

export function resolveAssumption(
    sessionId: string,
    assumptionId: string,
    userId: string,
    body: { resolved: boolean; resolution?: string }
) {
    return Effect.gen(function* () {
        const session = yield* getSessionById(sessionId, userId);
        if (!session) {
            return yield* Effect.fail(new NotFoundError({ resource: "session" }));
        }
        return yield* resolveAssumptionRepo(assumptionId, body);
    });
}

export function addRequirementRef(
    sessionId: string,
    userId: string,
    requirementId: string,
    stepsAddressed?: number[]
) {
    return Effect.gen(function* () {
        const session = yield* getSessionById(sessionId, userId);
        if (!session) {
            return yield* Effect.fail(new NotFoundError({ resource: "session" }));
        }
        return yield* addRequirementRefRepo(sessionId, requirementId, stepsAddressed ?? []);
    });
}

export function completeSession(sessionId: string, userId: string, prUrl?: string) {
    return Effect.gen(function* () {
        const session = yield* getSessionById(sessionId, userId);
        if (!session) {
            return yield* Effect.fail(new NotFoundError({ resource: "session" }));
        }

        const detail = yield* getSessionDetail(sessionId);

        // Fetch all requirements and conventions for the session's codebase
        const reqResult = yield* listRequirements({
            userId,
            codebaseId: session.codebaseId ?? undefined,
            limit: 100,
            offset: 0
        });

        const allChunks = yield* listChunks({
            userId,
            codebaseId: session.codebaseId ?? undefined,
            limit: 100,
            offset: 0
        });

        const allConventions = allChunks.chunks
            .filter(c => c.rationale)
            .map(c => ({ id: c.id, title: c.title }));

        const allRequirements = reqResult.requirements.map(r => ({
            id: r.id,
            title: r.title,
            status: r.status,
            steps: r.steps as unknown[]
        }));

        const completedAt = new Date();
        const reviewBrief = generateReviewBrief({
            session: { title: session.title, createdAt: session.createdAt, completedAt },
            chunkRefs: detail.chunkRefs.map(r => ({
                chunkId: r.chunkId,
                chunkTitle: r.chunkTitle,
                reason: r.reason
            })),
            assumptions: detail.assumptions.map(a => ({
                id: a.id,
                description: a.description
            })),
            requirementRefs: detail.requirementRefs.map(r => ({
                requirementId: r.requirementId,
                requirementTitle: r.requirementTitle,
                requirementStatus: r.requirementStatus,
                totalSteps: r.totalSteps,
                stepsAddressed: r.stepsAddressed as number[]
            })),
            allRequirements,
            allConventions
        });

        const updated = yield* updateSession(sessionId, userId, {
            status: "completed",
            completedAt,
            prUrl,
            reviewBrief
        });

        return updated;
    });
}

export function reviewSession(
    sessionId: string,
    userId: string,
    requirementStatuses?: Array<{ requirementId: string; status: string }>
) {
    return Effect.gen(function* () {
        const session = yield* getSessionById(sessionId, userId);
        if (!session) {
            return yield* Effect.fail(new NotFoundError({ resource: "session" }));
        }

        const updated = yield* updateSession(sessionId, userId, {
            status: "reviewed",
            reviewedAt: new Date()
        });

        if (requirementStatuses) {
            for (const rs of requirementStatuses) {
                yield* updateRequirementStatus(rs.requirementId, userId, rs.status);
            }
        }

        return updated;
    });
}
