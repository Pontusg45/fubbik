import { Plus } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SortableList } from "@/components/ui/sortable-list";
import { Textarea } from "@/components/ui/textarea";
import { useApiMutation } from "@/hooks/use-api-mutation";
import { api } from "@/utils/api";

import { PlanTaskCard, type Task } from "./plan-task-card";

export interface TaskDependency {
    id: string;
    taskId: string;
    dependsOnTaskId: string;
}

export interface PlanTasksSectionProps {
    planId: string;
    tasks: Task[];
    dependencies: TaskDependency[];
    onUpdate: () => void;
}

export function PlanTasksSection({ planId, tasks, dependencies, onUpdate }: PlanTasksSectionProps) {
    const [adding, setAdding] = useState(false);
    const [draftTitle, setDraftTitle] = useState("");
    const [draftDescription, setDraftDescription] = useState("");
    const [showDescription, setShowDescription] = useState(false);

    const reorderMutation = useApiMutation<unknown, { taskIds: string[] }>({
        mutationFn: async ({ taskIds }) => await (api.api as any).plans[planId].tasks.reorder.post({ taskIds }),
        successToast: false,
        errorToast: "Failed to reorder tasks",
        onSuccess: () => onUpdate(),
    });

    const addMutation = useApiMutation<unknown, { title: string; description: string }>({
        mutationFn: async ({ title, description }) => {
            const body: Record<string, unknown> = { title };
            if (description) body.description = description;
            return await (api.api as any).plans[planId].tasks.post(body);
        },
        successToast: false,
        errorToast: "Failed to add task",
        onSuccess: () => {
            setDraftTitle("");
            setDraftDescription("");
            setShowDescription(false);
            setAdding(false);
            onUpdate();
        },
    });

    const submit = () => {
        const title = draftTitle.trim();
        if (!title) return;
        addMutation.mutate({ title, description: draftDescription.trim() });
    };
    const cancel = () => {
        setAdding(false);
        setDraftTitle("");
        setDraftDescription("");
        setShowDescription(false);
    };

    // Build per-task dependency lookups.
    const dependsOnByTask = new Map<string, TaskDependency[]>();
    const dependentsByTask = new Map<string, TaskDependency[]>();
    for (const d of dependencies) {
        if (!dependsOnByTask.has(d.taskId)) dependsOnByTask.set(d.taskId, []);
        dependsOnByTask.get(d.taskId)!.push(d);
        if (!dependentsByTask.has(d.dependsOnTaskId)) dependentsByTask.set(d.dependsOnTaskId, []);
        dependentsByTask.get(d.dependsOnTaskId)!.push(d);
    }

    const doneCount = tasks.filter(t => t.status === "done").length;

    return (
        <section className="space-y-2">
            <div className="flex items-center justify-between">
                <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Tasks <span className="ml-1 font-mono text-muted-foreground/60">{doneCount}/{tasks.length} done</span>
                </h2>
                <Button size="sm" variant="ghost" onClick={() => setAdding(a => !a)} data-plan-add-task>
                    <Plus className="size-3.5" />
                    Add task
                </Button>
            </div>
            {adding && (
                <div className="rounded-md border bg-card p-2 space-y-2">
                    <div className="flex gap-2">
                        <Input
                            autoFocus
                            placeholder="Task title — Enter to add, Shift+Enter for description"
                            value={draftTitle}
                            onChange={e => setDraftTitle(e.target.value)}
                            onKeyDown={e => {
                                if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); submit(); }
                                else if (e.key === "Enter" && e.shiftKey) { e.preventDefault(); setShowDescription(true); }
                                else if (e.key === "Escape") cancel();
                            }}
                            className="h-8 text-sm"
                        />
                        <Button size="sm" onClick={submit} disabled={!draftTitle.trim() || addMutation.isPending}>
                            {addMutation.isPending ? "Adding..." : "Add"}
                        </Button>
                    </div>
                    {showDescription ? (
                        <Textarea
                            placeholder="Optional description (markdown)"
                            value={draftDescription}
                            onChange={e => setDraftDescription(e.target.value)}
                            rows={3}
                            className="text-sm"
                        />
                    ) : (
                        <button
                            type="button"
                            onClick={() => setShowDescription(true)}
                            className="text-xs text-muted-foreground hover:text-foreground underline-offset-2 hover:underline"
                        >
                            + add description
                        </button>
                    )}
                </div>
            )}
            {tasks.length === 0 && !adding ? (
                <div className="rounded-md border border-dashed p-4 text-center text-xs text-muted-foreground">
                    No tasks yet. Add the first one to start executing.
                </div>
            ) : (
                <div className="space-y-2">
                    <SortableList
                        items={tasks}
                        getId={t => t.id}
                        onReorder={taskIds => reorderMutation.mutate({ taskIds })}
                        renderItem={(t, { dragHandleProps }) => (
                            <PlanTaskCard
                                planId={planId}
                                task={t}
                                allTasks={tasks}
                                dependsOn={dependsOnByTask.get(t.id) ?? []}
                                dependentCount={(dependentsByTask.get(t.id) ?? []).length}
                                dragHandleProps={dragHandleProps as React.HTMLAttributes<HTMLDivElement>}
                                onUpdate={onUpdate}
                            />
                        )}
                    />
                </div>
            )}
        </section>
    );
}
