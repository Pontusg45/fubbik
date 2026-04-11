import { useMutation } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, X } from "lucide-react";
import { useState } from "react";

import { Checkbox } from "@/components/ui/checkbox";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export type TaskStatus = "pending" | "in_progress" | "done" | "skipped" | "blocked";

export interface Task {
    id: string;
    title: string;
    description: string | null;
    acceptanceCriteria: string[];
    status: TaskStatus;
    chunks: Array<{ id: string; chunkId: string; relation: string }>;
}

export interface PlanTaskCardProps {
    planId: string;
    task: Task;
    onUpdate: () => void;
}

export function PlanTaskCard({ planId, task, onUpdate }: PlanTaskCardProps) {
    const [expanded, setExpanded] = useState(false);

    const updateMutation = useMutation({
        mutationFn: async (patch: Record<string, unknown>) =>
            unwrapEden(await (api.api as any).plans[planId].tasks[task.id].patch(patch)),
        onSuccess: () => onUpdate(),
    });

    const deleteMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(await (api.api as any).plans[planId].tasks[task.id].delete()),
        onSuccess: () => onUpdate(),
    });

    const toggleDone = () => {
        updateMutation.mutate({ status: task.status === "done" ? "pending" : "done" });
    };

    return (
        <div className="rounded-md border bg-card">
            <div className="flex items-start gap-3 p-3">
                <Checkbox checked={task.status === "done"} onCheckedChange={toggleDone} className="mt-0.5" />
                <button type="button" onClick={() => setExpanded(e => !e)} className="flex-1 text-left">
                    <div className="flex items-center gap-2">
                        <span className={`font-medium ${task.status === "done" ? "line-through text-muted-foreground" : ""}`}>
                            {task.title}
                        </span>
                        {task.status === "blocked" && (
                            <span className="text-[9px] uppercase text-amber-500">blocked</span>
                        )}
                        {task.status === "in_progress" && (
                            <span className="text-[9px] uppercase text-blue-500">in progress</span>
                        )}
                    </div>
                    {task.description && !expanded && (
                        <div className="mt-1 line-clamp-1 text-xs text-muted-foreground">{task.description}</div>
                    )}
                </button>
                <button type="button" onClick={() => setExpanded(e => !e)} className="text-muted-foreground">
                    {expanded ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                </button>
                <button
                    type="button"
                    onClick={() => deleteMutation.mutate()}
                    className="text-muted-foreground hover:text-foreground"
                    aria-label="Delete task"
                >
                    <X className="size-3" />
                </button>
            </div>
            {expanded && (
                <div className="border-t px-3 py-2 text-xs">
                    {task.description && (
                        <div className="mb-2 whitespace-pre-wrap">{task.description}</div>
                    )}
                    {task.acceptanceCriteria.length > 0 && (
                        <div className="mb-2 space-y-1">
                            <div className="text-[10px] uppercase text-muted-foreground">Acceptance</div>
                            {task.acceptanceCriteria.map((ac, i) => (
                                <div key={i} className="flex items-start gap-2">
                                    <span className="mt-[2px] size-3 shrink-0 rounded border" />
                                    <span>{ac}</span>
                                </div>
                            ))}
                        </div>
                    )}
                    {task.chunks.length > 0 && (
                        <div className="mb-2">
                            <div className="text-[10px] uppercase text-muted-foreground">Chunks</div>
                            <div className="flex flex-wrap gap-1">
                                {task.chunks.map(c => (
                                    <span key={c.id} className="rounded bg-muted px-2 py-0.5 font-mono text-[10px]">
                                        {c.relation}:{c.chunkId.slice(0, 8)}
                                    </span>
                                ))}
                            </div>
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
