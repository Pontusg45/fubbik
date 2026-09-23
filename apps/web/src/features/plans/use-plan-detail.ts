import type { components } from "@fubbik/client";
import { useQuery } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import type { AnalyzeGroups } from "./plan-analyze-section";
import type { Task, TaskStatus } from "./plan-task-card";

export type PlanDetail = components["schemas"]["PlanDetail"];
export type PlanDetailView = Omit<PlanDetail, "tasks" | "analyze"> & { tasks: Task[]; analyze: AnalyzeGroups };

const TASK_STATUSES = new Set<TaskStatus>(["pending", "in_progress", "done", "skipped", "blocked"]);

function taskStatus(value: string): TaskStatus {
    if (TASK_STATUSES.has(value as TaskStatus)) return value as TaskStatus;
    throw new Error(`Unknown plan task status: ${value}`);
}

function toPlanDetailView(detail: PlanDetail): PlanDetailView {
    const mapItems = (kind: keyof AnalyzeGroups): AnalyzeGroups["chunk"] =>
        detail.analyze[kind].map(item => ({
            ...item,
            kind,
            chunkId: item.chunkId ?? null,
            filePath: item.filePath ?? null,
            text: item.text ?? null
        }));
    const analyze: AnalyzeGroups = {
        chunk: mapItems("chunk"),
        file: mapItems("file"),
        risk: mapItems("risk"),
        assumption: mapItems("assumption"),
        question: mapItems("question")
    };

    return {
        ...detail,
        analyze,
        tasks: detail.tasks.map(task => ({
            ...task,
            description: task.description ?? null,
            status: taskStatus(task.status),
            chunks: task.chunks.map(chunk => ({
                ...chunk,
                chunkTitle: chunk.chunkTitle ?? null,
                chunkType: chunk.chunkType ?? null
            }))
        }))
    };
}

export function usePlanDetail(planId: string) {
    return useQuery<PlanDetailView>({
        queryKey: ["plan-detail", planId],
        queryFn: async () => toPlanDetailView(unwrapEden(await api.api.plans({ id: planId }).get()))
    });
}
