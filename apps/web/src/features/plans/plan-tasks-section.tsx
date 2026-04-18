import {
    DndContext,
    closestCenter,
    KeyboardSensor,
    PointerSensor,
    useSensor,
    useSensors,
    type DragEndEvent,
} from "@dnd-kit/core";
import {
    SortableContext,
    arrayMove,
    sortableKeyboardCoordinates,
    useSortable,
    verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useMutation } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

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
    const [localOrder, setLocalOrder] = useState(tasks);

    useEffect(() => { setLocalOrder(tasks); }, [tasks]);

    const sensors = useSensors(
        useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
        useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
    );

    const reorderMutation = useMutation({
        mutationFn: async (taskIds: string[]) =>
            unwrapEden(await (api.api as any).plans[planId].tasks.reorder.post({ taskIds })),
        onSuccess: () => onUpdate(),
        onError: () => {
            setLocalOrder(tasks);
            toast.error("Failed to reorder tasks");
        },
    });

    function onDragEnd(event: DragEndEvent) {
        const { active, over } = event;
        if (!over || active.id === over.id) return;
        const oldIndex = localOrder.findIndex(t => t.id === active.id);
        const newIndex = localOrder.findIndex(t => t.id === over.id);
        if (oldIndex === -1 || newIndex === -1) return;
        const next = arrayMove(localOrder, oldIndex, newIndex);
        setLocalOrder(next);
        reorderMutation.mutate(next.map(t => t.id));
    }

    const addMutation = useMutation({
        mutationFn: async () => {
            const body: Record<string, unknown> = { title: draftTitle.trim() };
            if (draftDescription.trim()) body.description = draftDescription.trim();
            return unwrapEden(await (api.api as any).plans[planId].tasks.post(body));
        },
        onSuccess: () => {
            setDraftTitle("");
            setDraftDescription("");
            setShowDescription(false);
            setAdding(false);
            onUpdate();
        },
    });

    const submit = () => { if (draftTitle.trim()) addMutation.mutate(); };
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
    const ids = localOrder.map(t => t.id);

    return (
        <section className="space-y-2">
            <div className="flex items-center justify-between">
                <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Tasks <span className="ml-1 font-mono text-muted-foreground/60">{doneCount}/{tasks.length} done</span>
                </h2>
                <Button size="sm" variant="ghost" onClick={() => setAdding(a => !a)}>
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
                <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
                    <SortableContext items={ids} strategy={verticalListSortingStrategy}>
                        <div className="space-y-2">
                            {localOrder.map(t => (
                                <SortableTaskRow
                                    key={t.id}
                                    task={t}
                                    planId={planId}
                                    allTasks={localOrder}
                                    dependsOn={dependsOnByTask.get(t.id) ?? []}
                                    dependentCount={(dependentsByTask.get(t.id) ?? []).length}
                                    onUpdate={onUpdate}
                                />
                            ))}
                        </div>
                    </SortableContext>
                </DndContext>
            )}
        </section>
    );
}

function SortableTaskRow({
    task,
    planId,
    allTasks,
    dependsOn,
    dependentCount,
    onUpdate,
}: {
    task: Task;
    planId: string;
    allTasks: Task[];
    dependsOn: TaskDependency[];
    dependentCount: number;
    onUpdate: () => void;
}) {
    const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: task.id });
    const style: React.CSSProperties = {
        transform: CSS.Transform.toString(transform),
        transition,
        opacity: isDragging ? 0.5 : undefined,
    };
    return (
        <div ref={setNodeRef} style={style}>
            <PlanTaskCard
                planId={planId}
                task={task}
                allTasks={allTasks}
                dependsOn={dependsOn}
                dependentCount={dependentCount}
                dragHandleProps={{ ...attributes, ...listeners }}
                onUpdate={onUpdate}
            />
        </div>
    );
}
