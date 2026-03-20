import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ChevronDown, ExternalLink, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface PlanStep {
    id: string;
    planId: string;
    description: string;
    status: string;
    order: number;
    parentStepId: string | null;
    note: string | null;
    chunkId: string | null;
}

interface PlanStepItemProps {
    step: PlanStep;
    planId: string;
}

const statusColors: Record<string, string> = {
    pending: "secondary",
    in_progress: "default",
    done: "default",
    skipped: "outline",
    blocked: "destructive"
};

export function PlanStepItem({ step, planId }: PlanStepItemProps) {
    const queryClient = useQueryClient();
    const [expanded, setExpanded] = useState(false);
    const [note, setNote] = useState(step.note ?? "");

    const updateMutation = useMutation({
        mutationFn: async (body: { status?: string; note?: string | null }) => {
            return unwrapEden(
                await api.api.plans({ id: planId }).steps({ stepId: step.id }).patch(body)
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["plan", planId] });
        },
        onError: () => {
            toast.error("Failed to update step");
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async () => {
            return unwrapEden(
                await api.api.plans({ id: planId }).steps({ stepId: step.id }).delete()
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["plan", planId] });
            toast.success("Step removed");
        },
        onError: () => {
            toast.error("Failed to remove step");
        }
    });

    const isDone = step.status === "done";

    function toggleDone() {
        updateMutation.mutate({ status: isDone ? "pending" : "done" });
    }

    function setStatus(status: string) {
        updateMutation.mutate({ status });
    }

    function saveNote() {
        updateMutation.mutate({ note: note.trim() || null });
    }

    return (
        <div className="border-b last:border-b-0">
            <div className="flex items-center gap-3 py-2 px-1">
                <input
                    type="checkbox"
                    checked={isDone}
                    onChange={toggleDone}
                    disabled={updateMutation.isPending}
                    className="size-4 shrink-0 rounded border"
                />
                <span className={`flex-1 text-sm ${isDone ? "text-muted-foreground line-through" : ""}`}>
                    {step.description}
                </span>
                {step.status !== "pending" && step.status !== "done" && (
                    <Badge variant={statusColors[step.status] as "default" | "secondary" | "outline" | "destructive"} size="sm">
                        {step.status.replace("_", " ")}
                    </Badge>
                )}
                {step.chunkId && (
                    <Link
                        to="/chunks/$chunkId"
                        params={{ chunkId: step.chunkId }}
                        className="text-muted-foreground hover:text-foreground"
                        title="Linked chunk"
                    >
                        <ExternalLink className="size-3.5" />
                    </Link>
                )}
                <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setExpanded(!expanded)}
                    className="size-7 p-0"
                >
                    <ChevronDown className={`size-3.5 transition-transform ${expanded ? "rotate-0" : "-rotate-90"}`} />
                </Button>
            </div>

            {expanded && (
                <div className="pb-3 pl-8 pr-1 space-y-2">
                    <div className="flex flex-wrap gap-1">
                        {["pending", "in_progress", "done", "skipped", "blocked"].map(s => (
                            <Button
                                key={s}
                                variant={step.status === s ? "default" : "outline"}
                                size="sm"
                                onClick={() => setStatus(s)}
                                disabled={updateMutation.isPending}
                                className="text-xs h-7"
                            >
                                {s.replace("_", " ")}
                            </Button>
                        ))}
                    </div>
                    <textarea
                        value={note}
                        onChange={e => setNote(e.target.value)}
                        onBlur={saveNote}
                        placeholder="Add a note..."
                        rows={2}
                        className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                    />
                    <div className="flex justify-end">
                        <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => deleteMutation.mutate()}
                            disabled={deleteMutation.isPending}
                            className="text-destructive hover:text-destructive"
                        >
                            <Trash2 className="mr-1 size-3.5" />
                            Remove
                        </Button>
                    </div>
                </div>
            )}
        </div>
    );
}
