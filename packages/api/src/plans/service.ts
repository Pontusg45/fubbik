import { Effect } from "effect";

import * as planRepo from "@fubbik/db/repository/plan";
import type {
    PlanAnalyzeKind,
    PlanStatus,
    PlanTaskChunkRelation,
} from "@fubbik/db/schema/plan";

import { NotFoundError, ValidationError } from "../errors";

const VALID_STATUSES: PlanStatus[] = ["draft", "analyzing", "ready", "in_progress", "completed", "archived"];
const VALID_ANALYZE_KINDS: PlanAnalyzeKind[] = ["chunk", "file", "risk", "assumption", "question"];
const VALID_TASK_RELATIONS: PlanTaskChunkRelation[] = ["context", "created", "modified"];

export interface CreatePlanInput {
    title: string;
    description?: string;
    codebaseId?: string;
    requirementIds?: string[];
    tasks?: Array<{ title: string; description?: string; acceptanceCriteria?: string[] }>;
}

export interface ListPlansInput {
    userId: string;
    codebaseId?: string;
    status?: string;
    requirementId?: string;
    includeArchived?: boolean;
}

export function listPlans(input: ListPlansInput) {
    if (input.status && !VALID_STATUSES.includes(input.status as PlanStatus)) {
        return Effect.fail(new ValidationError({ message: `Invalid status: ${input.status}` }));
    }
    return planRepo.listPlans({
        userId: input.userId,
        codebaseId: input.codebaseId,
        status: input.status as PlanStatus | undefined,
        requirementId: input.requirementId,
        includeArchived: input.includeArchived,
    });
}

export function getPlan(id: string) {
    return planRepo.getPlan(id).pipe(
        Effect.flatMap(plan =>
            plan ? Effect.succeed(plan) : Effect.fail(new NotFoundError({ resource: `Plan(${id})` })),
        ),
    );
}

/**
 * Full plan detail including requirements, analyze items grouped by kind,
 * tasks, task-chunk links, and dependencies.
 */
export function getPlanDetail(id: string) {
    return Effect.gen(function* () {
        const plan = yield* getPlan(id);
        const requirements = yield* planRepo.listPlanRequirements(id);
        const analyzeItems = yield* planRepo.listAnalyzeItems(id);
        const tasks = yield* planRepo.listTasks(id);
        const dependencies = yield* planRepo.listTaskDependencies(id);

        const analyze: Record<PlanAnalyzeKind, typeof analyzeItems> = {
            chunk: [],
            file: [],
            risk: [],
            assumption: [],
            question: [],
        };
        for (const item of analyzeItems) {
            if (VALID_ANALYZE_KINDS.includes(item.kind as PlanAnalyzeKind)) {
                analyze[item.kind as PlanAnalyzeKind].push(item);
            }
        }

        const taskChunks = yield* Effect.all(tasks.map(t => planRepo.listTaskChunks(t.id)));
        const tasksWithChunks = tasks.map((t, i) => ({ ...t, chunks: taskChunks[i] ?? [] }));

        return { plan, requirements, analyze, tasks: tasksWithChunks, dependencies };
    });
}

export function createPlan(userId: string, input: CreatePlanInput) {
    return Effect.gen(function* () {
        if (!input.title.trim()) {
            return yield* Effect.fail(new ValidationError({ message: "Title is required" }));
        }
        const created = yield* planRepo.createPlan({
            id: crypto.randomUUID(),
            title: input.title.trim(),
            description: input.description ?? null,
            codebaseId: input.codebaseId ?? null,
            userId,
            status: "draft",
        });
        if (input.requirementIds) {
            for (const rid of input.requirementIds) {
                yield* planRepo.addPlanRequirement(created.id, rid);
            }
        }
        if (input.tasks) {
            for (const t of input.tasks) {
                yield* planRepo.createTask({
                    id: crypto.randomUUID(),
                    planId: created.id,
                    title: t.title,
                    description: t.description ?? null,
                    acceptanceCriteria: t.acceptanceCriteria ?? [],
                    status: "pending",
                });
            }
        }
        return created;
    });
}

export interface UpdatePlanInput {
    title?: string;
    description?: string | null;
    status?: string;
    codebaseId?: string | null;
}

export function updatePlan(id: string, input: UpdatePlanInput) {
    return Effect.gen(function* () {
        if (input.status && !VALID_STATUSES.includes(input.status as PlanStatus)) {
            return yield* Effect.fail(new ValidationError({ message: `Invalid status: ${input.status}` }));
        }
        const existing = yield* getPlan(id);
        const patch: Parameters<typeof planRepo.updatePlan>[1] = {};
        if (input.title !== undefined) patch.title = input.title;
        if (input.description !== undefined) patch.description = input.description;
        if (input.codebaseId !== undefined) patch.codebaseId = input.codebaseId;
        if (input.status !== undefined) {
            patch.status = input.status as PlanStatus;
            if (input.status === "completed" && existing.status !== "completed") {
                patch.completedAt = new Date();
            } else if (input.status !== "completed" && existing.status === "completed") {
                patch.completedAt = null;
            }
        }
        return yield* planRepo.updatePlan(id, patch);
    });
}

export function deletePlan(id: string) {
    return Effect.gen(function* () {
        yield* getPlan(id);
        yield* planRepo.deletePlan(id);
    });
}

export { VALID_STATUSES, VALID_ANALYZE_KINDS, VALID_TASK_RELATIONS };
