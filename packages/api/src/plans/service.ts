import {
    addPlanChunkRef as addChunkRefRepo,
    createPlan as createPlanRepo,
    createStep as createStepRepo,
    deletePlan as deletePlanRepo,
    deleteStep as deleteStepRepo,
    getChunkRefsForPlan,
    getPlanById,
    getStepsForPlan,
    listPlans as listPlansRepo,
    removePlanChunkRef as removeChunkRefRepo,
    updatePlan as updatePlanRepo,
    updateStep as updateStepRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";

const VALID_PLAN_STATUSES = ["draft", "active", "completed", "archived"];
const VALID_STEP_STATUSES = ["pending", "in_progress", "done", "skipped", "blocked"];
const VALID_REF_RELATIONS = ["context", "created", "modified"];

export function listPlans(userId: string, codebaseId?: string, status?: string) {
    return Effect.gen(function* () {
        if (status && !VALID_PLAN_STATUSES.includes(status)) {
            return yield* Effect.fail(
                new ValidationError({ message: `Invalid plan status: ${status}` })
            );
        }
        return yield* listPlansRepo(userId, codebaseId, status);
    });
}

export function getPlanDetail(id: string, userId: string) {
    return Effect.gen(function* () {
        const found = yield* getPlanById(id, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        const steps = yield* getStepsForPlan(id);
        const chunkRefs = yield* getChunkRefsForPlan(id);

        const totalSteps = steps.length;
        const doneCount = steps.filter(s => s.status === "done").length;

        return {
            ...found,
            steps,
            chunkRefs,
            progress: { doneCount, totalSteps }
        };
    });
}

export function createPlan(
    userId: string,
    body: {
        title: string;
        description?: string;
        status?: string;
        codebaseId?: string;
        steps?: Array<{ description: string; order?: number; parentStepId?: string; note?: string; chunkId?: string }>;
    }
) {
    return Effect.gen(function* () {
        if (body.status && !VALID_PLAN_STATUSES.includes(body.status)) {
            return yield* Effect.fail(
                new ValidationError({ message: `Invalid plan status: ${body.status}` })
            );
        }

        const planId = crypto.randomUUID();
        const created = yield* createPlanRepo({
            id: planId,
            title: body.title,
            description: body.description,
            status: body.status,
            userId,
            codebaseId: body.codebaseId
        });

        const steps = [];
        if (body.steps && body.steps.length > 0) {
            for (let i = 0; i < body.steps.length; i++) {
                const stepBody = body.steps[i]!;
                const step = yield* createStepRepo({
                    id: crypto.randomUUID(),
                    planId,
                    description: stepBody.description,
                    order: stepBody.order ?? i,
                    parentStepId: stepBody.parentStepId,
                    note: stepBody.note,
                    chunkId: stepBody.chunkId
                });
                steps.push(step);
            }
        }

        return { ...created, steps, chunkRefs: [], progress: { doneCount: 0, totalSteps: steps.length } };
    });
}

export function updatePlan(
    id: string,
    userId: string,
    body: {
        title?: string;
        description?: string | null;
        status?: string;
        codebaseId?: string | null;
    }
) {
    return Effect.gen(function* () {
        if (body.status && !VALID_PLAN_STATUSES.includes(body.status)) {
            return yield* Effect.fail(
                new ValidationError({ message: `Invalid plan status: ${body.status}` })
            );
        }

        const found = yield* getPlanById(id, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        const updated = yield* updatePlanRepo(id, userId, body);
        if (!updated) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));
        return updated;
    });
}

export function deletePlan(id: string, userId: string) {
    return deletePlanRepo(id, userId).pipe(
        Effect.flatMap(deleted =>
            deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Plan" }))
        )
    );
}

// ── Steps ──────────────────────────────────────────────────────────

export function addStep(
    planId: string,
    userId: string,
    body: {
        description: string;
        order?: number;
        parentStepId?: string;
        note?: string;
        chunkId?: string;
    }
) {
    return Effect.gen(function* () {
        const found = yield* getPlanById(planId, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        // Auto-assign order if not provided
        let order = body.order;
        if (order === undefined) {
            const steps = yield* getStepsForPlan(planId);
            order = steps.length;
        }

        return yield* createStepRepo({
            id: crypto.randomUUID(),
            planId,
            description: body.description,
            order,
            parentStepId: body.parentStepId,
            note: body.note,
            chunkId: body.chunkId
        });
    });
}

export function updateStep(
    planId: string,
    stepId: string,
    userId: string,
    body: {
        description?: string;
        status?: string;
        order?: number;
        parentStepId?: string | null;
        note?: string | null;
        chunkId?: string | null;
    }
) {
    return Effect.gen(function* () {
        if (body.status && !VALID_STEP_STATUSES.includes(body.status)) {
            return yield* Effect.fail(
                new ValidationError({ message: `Invalid step status: ${body.status}` })
            );
        }

        const found = yield* getPlanById(planId, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        const updated = yield* updateStepRepo(stepId, planId, body);
        if (!updated) return yield* Effect.fail(new NotFoundError({ resource: "PlanStep" }));
        return updated;
    });
}

export function deleteStep(planId: string, stepId: string, userId: string) {
    return Effect.gen(function* () {
        const found = yield* getPlanById(planId, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        const deleted = yield* deleteStepRepo(stepId, planId);
        if (!deleted) return yield* Effect.fail(new NotFoundError({ resource: "PlanStep" }));
        return deleted;
    });
}

// ── Step Reorder ──────────────────────────────────────────────────

export function reorderSteps(planId: string, userId: string, stepIds: string[]) {
    return Effect.gen(function* () {
        const found = yield* getPlanById(planId, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        for (let i = 0; i < stepIds.length; i++) {
            yield* updateStepRepo(stepIds[i]!, planId, { order: i });
        }
        return yield* getStepsForPlan(planId);
    });
}

// ── Chunk Refs ─────────────────────────────────────────────────────

export function addPlanChunkRef(
    planId: string,
    userId: string,
    body: {
        chunkId: string;
        relation?: string;
    }
) {
    return Effect.gen(function* () {
        if (body.relation && !VALID_REF_RELATIONS.includes(body.relation)) {
            return yield* Effect.fail(
                new ValidationError({ message: `Invalid chunk ref relation: ${body.relation}` })
            );
        }

        const found = yield* getPlanById(planId, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        return yield* addChunkRefRepo({
            id: crypto.randomUUID(),
            planId,
            chunkId: body.chunkId,
            relation: body.relation
        });
    });
}

export function removePlanChunkRef(planId: string, refId: string, userId: string) {
    return Effect.gen(function* () {
        const found = yield* getPlanById(planId, userId);
        if (!found) return yield* Effect.fail(new NotFoundError({ resource: "Plan" }));

        const deleted = yield* removeChunkRefRepo(refId, planId);
        if (!deleted) return yield* Effect.fail(new NotFoundError({ resource: "PlanChunkRef" }));
        return deleted;
    });
}
