import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ArrowRight } from "lucide-react";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";
import { PlanStatusPill } from "@/features/plans/plan-status-pill";
import type { PlanStatusValue } from "@/features/plans/plan-status-pill";
import type { TaskStatus } from "@/features/plans/plan-task-card";

interface Task {
    id: string;
    title: string;
    status: TaskStatus;
}

interface Plan {
    id: string;
    title: string;
    status: PlanStatusValue;
}

function taskIcon(status: TaskStatus) {
    switch (status) {
        case "done":
            return <span className="text-emerald-500 font-bold">✓</span>;
        case "in_progress":
            return <span className="text-blue-500 font-bold">→</span>;
        case "blocked":
            return <span className="text-amber-500 font-bold">✗</span>;
        default:
            return <span className="text-muted-foreground">○</span>;
    }
}

export function ActivePlanCard() {
    const queryClient = useQueryClient();

    const inProgressQuery = useQuery({
        queryKey: ["plans-in-progress"],
        queryFn: async () =>
            unwrapEden(await api.api.plans.get({ query: { status: "in_progress" } as any })),
    });

    const readyQuery = useQuery({
        queryKey: ["plans-ready"],
        queryFn: async () =>
            unwrapEden(await api.api.plans.get({ query: { status: "ready" } as any })),
        enabled: !inProgressQuery.isLoading && !(inProgressQuery.data as any)?.length,
    });

    const inProgressPlans = (inProgressQuery.data as any) ?? [];
    const readyPlans = (readyQuery.data as any) ?? [];
    const activePlan: Plan | undefined = inProgressPlans[0] ?? readyPlans[0];

    const detailQuery = useQuery({
        queryKey: ["plan-detail", activePlan?.id],
        queryFn: async () =>
            unwrapEden(await (api.api as any).plans[activePlan!.id].get()),
        enabled: !!activePlan?.id,
    });

    const updateTaskMutation = useMutation({
        mutationFn: async ({ taskId, status }: { taskId: string; status: TaskStatus }) =>
            unwrapEden(
                await (api.api as any).plans[activePlan!.id].tasks[taskId].patch({ status })
            ),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["plan-detail", activePlan?.id] });
        },
    });

    const isLoading = inProgressQuery.isLoading || (readyQuery.isLoading && !inProgressPlans.length);

    if (isLoading) return null;

    if (!activePlan) {
        return (
            <div className="rounded-lg border border-indigo-500/20 bg-indigo-500/5 px-4 py-3">
                <p className="text-sm text-muted-foreground">
                    No active plan —{" "}
                    <Link to="/plans/new" className="text-indigo-400 hover:underline inline-flex items-center gap-1">
                        Start one <ArrowRight className="size-3" />
                    </Link>
                </p>
            </div>
        );
    }

    const detail = detailQuery.data as any;
    const tasks: Task[] = detail?.tasks ?? [];
    const doneCount = tasks.filter((t) => t.status === "done").length;
    const total = tasks.length;
    const progress = total > 0 ? (doneCount / total) * 100 : 0;
    const visibleTasks = tasks.slice(0, 8);

    return (
        <div className="rounded-lg border border-indigo-500/20 bg-indigo-500/5 p-4">
            <div className="mb-3 flex items-center justify-between gap-2">
                <div className="min-w-0 flex items-center gap-2">
                    <PlanStatusPill status={activePlan.status} />
                    <Link
                        to="/plans/$planId"
                        params={{ planId: activePlan.id }}
                        className="truncate font-semibold text-sm hover:underline"
                    >
                        {activePlan.title}
                    </Link>
                </div>
                {total > 0 && (
                    <span className="shrink-0 font-mono text-xs text-muted-foreground">
                        {doneCount}/{total}
                    </span>
                )}
            </div>

            {total > 0 && (
                <div className="mb-3 h-1.5 w-full overflow-hidden rounded-full bg-indigo-500/15">
                    <div
                        className="h-full rounded-full bg-indigo-500 transition-all"
                        style={{ width: `${progress}%` }}
                    />
                </div>
            )}

            {visibleTasks.length > 0 && (
                <ul className="space-y-1">
                    {visibleTasks.map((task) => (
                        <li key={task.id} className="flex items-center gap-2">
                            <button
                                type="button"
                                className="shrink-0 w-4 text-center leading-none"
                                onClick={() =>
                                    updateTaskMutation.mutate({
                                        taskId: task.id,
                                        status: task.status === "done" ? "pending" : "done",
                                    })
                                }
                                disabled={updateTaskMutation.isPending}
                                aria-label={task.status === "done" ? "Mark pending" : "Mark done"}
                            >
                                {taskIcon(task.status)}
                            </button>
                            <span
                                className={`text-sm truncate ${task.status === "done" ? "line-through text-muted-foreground" : ""}`}
                            >
                                {task.title}
                            </span>
                        </li>
                    ))}
                    {tasks.length > 8 && (
                        <li className="text-xs text-muted-foreground pl-6">
                            +{tasks.length - 8} more —{" "}
                            <Link to="/plans/$planId" params={{ planId: activePlan.id }} className="hover:underline">
                                view all
                            </Link>
                        </li>
                    )}
                </ul>
            )}

            {detailQuery.isLoading && (
                <p className="text-xs text-muted-foreground">Loading tasks…</p>
            )}
        </div>
    );
}
