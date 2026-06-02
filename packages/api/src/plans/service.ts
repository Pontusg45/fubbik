import * as planRepo from "@fubbik/db/repository/plan";
import type { PlanAnalyzeKind, PlanStatus, PlanTaskChunkRelation } from "@fubbik/db/schema/plan";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";

const VALID_STATUSES: PlanStatus[] = ["draft", "analyzing", "ready", "in_progress", "completed", "archived"];
const VALID_ANALYZE_KINDS: PlanAnalyzeKind[] = ["chunk", "file", "risk", "assumption", "question"];
const VALID_TASK_RELATIONS: PlanTaskChunkRelation[] = ["context", "created", "modified"];

function isPlanStatus(s: string): s is PlanStatus {
    return (VALID_STATUSES as readonly string[]).includes(s);
}

function isAnalyzeKind(s: string): s is PlanAnalyzeKind {
    return (VALID_ANALYZE_KINDS as readonly string[]).includes(s);
}

export interface CreatePlanInput {
    title: string;
    description?: string;
    spaceId?: string;
    requirementIds?: string[];
    tasks?: Array<{ title: string; description?: string; acceptanceCriteria?: string[] }>;
    metadata?: Record<string, unknown>;
}

export interface ListPlansInput {
    userId: string;
    spaceId?: string;
    status?: string;
    requirementId?: string;
    includeArchived?: boolean;
}

export function listPlans(input: ListPlansInput) {
    return Effect.gen(function* () {
        if (input.status && !isPlanStatus(input.status)) {
            return yield* Effect.fail(new ValidationError({ message: `Invalid status: ${input.status}` }));
        }
        return yield* planRepo.listPlansWithRollups({
            userId: input.userId,
            spaceId: input.spaceId,
            status: input.status as PlanStatus | undefined,
            requirementId: input.requirementId,
            includeArchived: input.includeArchived
        });
    });
}

export function duplicatePlan(userId: string, sourceId: string) {
    return Effect.gen(function* () {
        yield* getPlan(sourceId); // 404 if missing
        return yield* planRepo.duplicatePlan(sourceId, userId);
    });
}

export function getPlan(id: string) {
    return planRepo
        .getPlan(id)
        .pipe(Effect.flatMap(plan => (plan ? Effect.succeed(plan) : Effect.fail(new NotFoundError({ resource: `Plan(${id})` })))));
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
            question: []
        };
        for (const item of analyzeItems) {
            if (isAnalyzeKind(item.kind)) {
                analyze[item.kind].push(item);
            }
        }

        const taskChunks = yield* Effect.all(tasks.map(t => planRepo.listTaskChunksWithTitles(t.id)));
        const tasksWithChunks = tasks.map((t, i) => ({
            ...t,
            acceptanceCriteria: normaliseAcceptanceCriteria(t.acceptanceCriteria),
            chunks: taskChunks[i] ?? []
        }));

        return { plan, requirements, analyze, tasks: tasksWithChunks, dependencies };
    });
}

export interface AcceptanceCriterion {
    text: string;
    done: boolean;
}

/**
 * acceptanceCriteria was originally `string[]`. We keep reading either the
 * legacy shape or the new `{text, done}[]` shape and always return the object
 * shape to clients. Write-side callers must send the object shape.
 */
export function normaliseAcceptanceCriteria(raw: unknown): AcceptanceCriterion[] {
    if (!Array.isArray(raw)) return [];
    return raw.map(item => {
        if (typeof item === "string") return { text: item, done: false };
        if (item && typeof item === "object" && "text" in item) {
            return {
                text: String(item.text ?? ""),
                done: Boolean("done" in item ? item.done : false)
            };
        }
        return { text: "", done: false };
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
            spaceId: input.spaceId ?? null,
            userId,
            status: "draft",
            metadata: input.metadata ?? {}
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
                    acceptanceCriteria: normaliseAcceptanceCriteria(t.acceptanceCriteria ?? []),
                    status: "pending"
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
    spaceId?: string | null;
    metadata?: Record<string, unknown>;
}

export function updatePlan(id: string, input: UpdatePlanInput) {
    return Effect.gen(function* () {
        if (input.status && !isPlanStatus(input.status)) {
            return yield* Effect.fail(new ValidationError({ message: `Invalid status: ${input.status}` }));
        }
        const existing = yield* getPlan(id);
        const patch: Parameters<typeof planRepo.updatePlan>[1] = {};
        if (input.title !== undefined) patch.title = input.title;
        if (input.description !== undefined) patch.description = input.description;
        if (input.spaceId !== undefined) patch.spaceId = input.spaceId;
        if (input.metadata !== undefined) patch.metadata = input.metadata;
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

export function updateTask(taskId: string, data: { status: string }) {
    return planRepo.updateTask(taskId, data);
}
