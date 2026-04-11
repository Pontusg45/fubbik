import { useMutation } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { PlanTaskCard, type Task } from "./plan-task-card";

export interface PlanTasksSectionProps {
    planId: string;
    tasks: Task[];
    onUpdate: () => void;
}

export function PlanTasksSection({ planId, tasks, onUpdate }: PlanTasksSectionProps) {
    const [adding, setAdding] = useState(false);
    const [draftTitle, setDraftTitle] = useState("");

    const addMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(await (api.api as any).plans[planId].tasks.post({ title: draftTitle.trim() })),
        onSuccess: () => {
            setDraftTitle("");
            setAdding(false);
            onUpdate();
        },
    });

    const doneCount = tasks.filter(t => t.status === "done").length;

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
                <div className="flex gap-2 rounded-md border bg-card p-2">
                    <Input
                        autoFocus
                        placeholder="Task title"
                        value={draftTitle}
                        onChange={e => setDraftTitle(e.target.value)}
                        onKeyDown={e => {
                            if (e.key === "Enter" && draftTitle.trim()) addMutation.mutate();
                            if (e.key === "Escape") {
                                setAdding(false);
                                setDraftTitle("");
                            }
                        }}
                        className="h-8 text-sm"
                    />
                    <Button size="sm" onClick={() => draftTitle.trim() && addMutation.mutate()}>Add</Button>
                </div>
            )}
            {tasks.length === 0 && !adding ? (
                <div className="rounded-md border border-dashed p-4 text-center text-xs text-muted-foreground">
                    No tasks yet. Add the first one to start executing.
                </div>
            ) : (
                <div className="space-y-2">
                    {tasks.map(t => (
                        <PlanTaskCard key={t.id} planId={planId} task={t} onUpdate={onUpdate} />
                    ))}
                </div>
            )}
        </section>
    );
}
