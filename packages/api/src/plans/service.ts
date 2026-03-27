import {
    addPlanChunkRef as addChunkRefRepo,
    createPlan as createPlanRepo,
    createStep as createStepRepo,
    deletePlan as deletePlanRepo,
    deleteStep as deleteStepRepo,
    getChunkRefsForPlan,
    getPlanById,
    getRequirementsByIds,
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

export const PLAN_TEMPLATES: Record<string, { title: string; description: string; steps: string[] }> = {
    "feature-dev": {
        title: "Feature Development",
        description: "Standard feature development workflow",
        steps: [
            "Understand requirements and acceptance criteria",
            "Review related existing chunks and conventions",
            "Design approach and identify affected files",
            "Implement core functionality",
            "Write tests",
            "Update documentation and knowledge chunks",
            "Self-review and refactor",
            "Create PR and request review"
        ]
    },
    "bug-fix": {
        title: "Bug Investigation & Fix",
        description: "Structured approach to debugging and fixing",
        steps: [
            "Reproduce the bug and document steps",
            "Identify root cause",
            "Check for related known issues in chunks",
            "Implement fix",
            "Write regression test",
            "Update relevant chunks if behavior changed",
            "Verify fix in context of related features"
        ]
    },
    "migration": {
        title: "Migration",
        description: "Database or dependency migration workflow",
        steps: [
            "Document current state and target state",
            "Identify all affected components",
            "Create migration script",
            "Test migration on staging/dev",
            "Create rollback plan",
            "Execute migration",
            "Verify data integrity",
            "Update knowledge chunks with new architecture"
        ]
    }
};

export function listPlanTemplates() {
    return Object.entries(PLAN_TEMPLATES).map(([key, tmpl]) => ({
        key,
        title: tmpl.title,
        description: tmpl.description,
        stepCount: tmpl.steps.length,
        steps: tmpl.steps
    }));
}

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

        // Batch-fetch requirement info for steps that have requirementId
        const reqIds = [...new Set(steps.filter(s => s.requirementId).map(s => s.requirementId!))];
        let reqMap = new Map<string, { title: string; status: string }>();
        if (reqIds.length > 0) {
            const reqs = yield* getRequirementsByIds(reqIds, userId);
            reqMap = new Map(reqs.map(r => [r.id, { title: r.title, status: r.status }]));
        }

        const enrichedSteps = steps.map(s => ({
            ...s,
            requirementTitle: s.requirementId ? reqMap.get(s.requirementId)?.title ?? null : null,
            requirementStatus: s.requirementId ? reqMap.get(s.requirementId)?.status ?? null : null
        }));

        const totalSteps = steps.length;
        const doneCount = steps.filter(s => s.status === "done").length;

        return {
            ...found,
            steps: enrichedSteps,
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
        template?: string;
        steps?: Array<{ description: string; order?: number; parentStepId?: string; note?: string; chunkId?: string; requirementId?: string; dependsOnStepId?: string }>;
    }
) {
    return Effect.gen(function* () {
        if (body.status && !VALID_PLAN_STATUSES.includes(body.status)) {
            return yield* Effect.fail(
                new ValidationError({ message: `Invalid plan status: ${body.status}` })
            );
        }

        // Apply template defaults if specified
        let title = body.title;
        let description = body.description;
        let steps = body.steps;

        if (body.template) {
            const tmpl = PLAN_TEMPLATES[body.template];
            if (!tmpl) {
                return yield* Effect.fail(
                    new ValidationError({
                        message: `Unknown plan template: ${body.template}. Available: ${Object.keys(PLAN_TEMPLATES).join(", ")}`
                    })
                );
            }
            // Template values are defaults; explicit values override
            if (!title || title === tmpl.title) title = tmpl.title;
            if (!description) description = tmpl.description;
            if (!steps || steps.length === 0) {
                steps = tmpl.steps.map((s, i) => ({ description: s, order: i }));
            }
        }

        const planId = crypto.randomUUID();
        const created = yield* createPlanRepo({
            id: planId,
            title,
            description,
            status: body.status,
            userId,
            codebaseId: body.codebaseId
        });

        const createdSteps = [];
        if (steps && steps.length > 0) {
            for (let i = 0; i < steps.length; i++) {
                const stepBody = steps[i]!;
                const step = yield* createStepRepo({
                    id: crypto.randomUUID(),
                    planId,
                    description: stepBody.description,
                    order: stepBody.order ?? i,
                    parentStepId: stepBody.parentStepId,
                    note: stepBody.note,
                    chunkId: stepBody.chunkId,
                    requirementId: stepBody.requirementId,
                    dependsOnStepId: stepBody.dependsOnStepId
                });
                createdSteps.push(step);
            }
        }

        return { ...created, steps: createdSteps, chunkRefs: [], progress: { doneCount: 0, totalSteps: createdSteps.length } };
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
        requirementId?: string;
        dependsOnStepId?: string;
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
            chunkId: body.chunkId,
            requirementId: body.requirementId,
            dependsOnStepId: body.dependsOnStepId
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
        requirementId?: string | null;
        dependsOnStepId?: string | null;
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

        // Auto-unblock: when a step is marked "done", unblock dependent steps
        if (body.status === "done") {
            const allSteps = yield* getStepsForPlan(planId);
            for (const s of allSteps) {
                if (s.dependsOnStepId === stepId && s.status === "blocked") {
                    yield* updateStepRepo(s.id, planId, { status: "pending" });
                }
            }
        }

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
