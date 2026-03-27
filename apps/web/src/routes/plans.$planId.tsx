import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, BarChart3, ClipboardList, List, MoreHorizontal, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger
} from "@/components/ui/dropdown-menu";
import { Skeleton } from "@/components/ui/skeleton";
import { PlanProgressBar } from "@/features/plans/plan-progress-bar";
import { PlanStepItem } from "@/features/plans/plan-step-item";
import { PlanTimeline } from "@/features/plans/plan-timeline";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/plans/$planId")({
    component: PlanDetail,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest access
        }
        return { session };
    }
});

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

interface PlanData {
    id: string;
    title: string;
    description: string | null;
    status: string;
    createdAt: Date;
    updatedAt: Date;
    steps: PlanStep[];
    chunkRefs: Array<{ id: string; chunkId: string; relation: string }>;
    progress: { doneCount: number; totalSteps: number };
}

const statusVariant: Record<string, "default" | "secondary" | "outline" | "destructive"> = {
    draft: "secondary",
    active: "default",
    completed: "outline",
    archived: "outline"
};

function PlanDetail() {
    const { planId } = Route.useParams();
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const [newStepText, setNewStepText] = useState("");
    const [showDeleteDialog, setShowDeleteDialog] = useState(false);
    const [viewMode, setViewMode] = useState<"list" | "timeline">("list");

    const { data: plan, isLoading, error } = useQuery({
        queryKey: ["plan", planId],
        queryFn: async () => {
            const { data, error } = await api.api.plans({ id: planId }).get();
            if (error) throw new Error("Failed to load plan");
            return data as PlanData;
        }
    });

    const updateStatusMutation = useMutation({
        mutationFn: async (status: string) => {
            return unwrapEden(await api.api.plans({ id: planId }).patch({ status }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["plan", planId] });
            queryClient.invalidateQueries({ queryKey: ["plans"] });
            toast.success("Plan status updated");
        },
        onError: () => {
            toast.error("Failed to update plan status");
        }
    });

    const addStepMutation = useMutation({
        mutationFn: async (description: string) => {
            return unwrapEden(
                await api.api.plans({ id: planId }).steps.post({ description })
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["plan", planId] });
            setNewStepText("");
            toast.success("Step added");
        },
        onError: () => {
            toast.error("Failed to add step");
        }
    });

    const reorderMutation = useMutation({
        mutationFn: async (stepIds: string[]) => {
            return unwrapEden(
                await api.api.plans({ id: planId }).steps.reorder.post({ stepIds })
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["plan", planId] });
        },
        onError: () => {
            toast.error("Failed to reorder steps");
        }
    });

    const moveStep = (index: number, direction: "up" | "down") => {
        if (!plan) return;
        const steps = [...plan.steps];
        const targetIndex = direction === "up" ? index - 1 : index + 1;
        if (targetIndex < 0 || targetIndex >= steps.length) return;
        [steps[index], steps[targetIndex]] = [steps[targetIndex]!, steps[index]!];
        reorderMutation.mutate(steps.map(s => s.id));
    };

    const deleteMutation = useMutation({
        mutationFn: async () => {
            return unwrapEden(await api.api.plans({ id: planId }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["plans"] });
            toast.success("Plan deleted");
            navigate({ to: "/plans" });
        },
        onError: () => {
            toast.error("Failed to delete plan");
        }
    });

    if (isLoading) {
        return (
            <div className="container mx-auto max-w-3xl px-4 py-8 space-y-4">
                <Skeleton className="h-8 w-48" />
                <Skeleton className="h-4 w-96" />
                <Skeleton className="h-32 w-full" />
            </div>
        );
    }

    if (error || !plan) {
        return (
            <div className="container mx-auto max-w-3xl px-4 py-8">
                <p className="text-muted-foreground">Plan not found.</p>
                <Button variant="ghost" size="sm" className="mt-4" render={<Link to="/plans" />}>
                    <ArrowLeft className="size-4" />
                    Back to Plans
                </Button>
            </div>
        );
    }

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6">
                <Button variant="ghost" size="sm" render={<Link to="/plans" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
            </div>

            {/* Header */}
            <div className="mb-6">
                <div className="flex items-center gap-3">
                    <ClipboardList className="size-5" />
                    <h1 className="text-2xl font-bold tracking-tight">{plan.title}</h1>
                    <Badge variant={statusVariant[plan.status] ?? "secondary"}>
                        {plan.status}
                    </Badge>
                </div>
                {plan.description && (
                    <p className="text-muted-foreground mt-2">{plan.description}</p>
                )}
            </div>

            {/* Progress */}
            {plan.progress.totalSteps > 0 && (
                <Card className="mb-6">
                    <CardPanel className="p-4">
                        <PlanProgressBar
                            doneCount={plan.progress.doneCount}
                            totalSteps={plan.progress.totalSteps}
                        />
                    </CardPanel>
                </Card>
            )}

            {/* Steps */}
            <Card className="mb-6">
                <CardPanel className="p-4">
                    <div className="mb-3 flex items-center justify-between">
                        <h2 className="text-sm font-medium">Steps</h2>
                        {plan.steps.length > 0 && (
                            <div className="flex items-center rounded-md border p-0.5">
                                <Button
                                    variant={viewMode === "list" ? "default" : "ghost"}
                                    size="sm"
                                    onClick={() => setViewMode("list")}
                                    className="h-6 px-2 text-xs"
                                >
                                    <List className="mr-1 size-3" />
                                    List
                                </Button>
                                <Button
                                    variant={viewMode === "timeline" ? "default" : "ghost"}
                                    size="sm"
                                    onClick={() => setViewMode("timeline")}
                                    className="h-6 px-2 text-xs"
                                >
                                    <BarChart3 className="mr-1 size-3" />
                                    Timeline
                                </Button>
                            </div>
                        )}
                    </div>
                    {plan.steps.length === 0 ? (
                        <p className="text-muted-foreground text-sm py-2">No steps yet.</p>
                    ) : viewMode === "timeline" ? (
                        <PlanTimeline steps={plan.steps} />
                    ) : (
                        <div className="border rounded-md">
                            {plan.steps.map((step, index) => (
                                <PlanStepItem
                                    key={step.id}
                                    step={step}
                                    planId={planId}
                                    isFirst={index === 0}
                                    isLast={index === plan.steps.length - 1}
                                    onMoveUp={() => moveStep(index, "up")}
                                    onMoveDown={() => moveStep(index, "down")}
                                />
                            ))}
                        </div>
                    )}

                    {/* Add step */}
                    <div className="mt-3 flex gap-2">
                        <Input
                            type="text"
                            value={newStepText}
                            onChange={e => setNewStepText(e.target.value)}
                            onKeyDown={e => {
                                if (e.key === "Enter" && newStepText.trim()) {
                                    e.preventDefault();
                                    addStepMutation.mutate(newStepText.trim());
                                }
                            }}
                            placeholder="Add a step..."
                            className="flex-1"
                        />
                        <Button
                            variant="outline"
                            size="sm"
                            onClick={() => {
                                if (newStepText.trim()) addStepMutation.mutate(newStepText.trim());
                            }}
                            disabled={addStepMutation.isPending || !newStepText.trim()}
                        >
                            <Plus className="mr-1 size-3.5" />
                            Add
                        </Button>
                    </div>
                </CardPanel>
            </Card>

            {/* Actions */}
            <div className="flex items-center gap-2">
                {plan.status === "draft" && (
                    <Button
                        size="sm"
                        onClick={() => updateStatusMutation.mutate("active")}
                        disabled={updateStatusMutation.isPending}
                    >
                        Activate Plan
                    </Button>
                )}
                {plan.status === "active" && (
                    <Button
                        size="sm"
                        onClick={() => updateStatusMutation.mutate("completed")}
                        disabled={updateStatusMutation.isPending}
                    >
                        Mark Complete
                    </Button>
                )}
                {(plan.status === "completed" || plan.status === "archived") && (
                    <Button
                        size="sm"
                        onClick={() => updateStatusMutation.mutate("active")}
                        disabled={updateStatusMutation.isPending}
                    >
                        Reactivate
                    </Button>
                )}

                <DropdownMenu>
                    <DropdownMenuTrigger
                        render={
                            <Button variant="outline" size="sm" className="size-8 p-0">
                                <MoreHorizontal className="size-4" />
                            </Button>
                        }
                    />
                    <DropdownMenuContent align="end">
                        {plan.status !== "draft" && (
                            <DropdownMenuItem
                                onClick={() => updateStatusMutation.mutate("draft")}
                                disabled={updateStatusMutation.isPending}
                            >
                                Move to Draft
                            </DropdownMenuItem>
                        )}
                        {plan.status !== "archived" && (
                            <DropdownMenuItem
                                onClick={() => updateStatusMutation.mutate("archived")}
                                disabled={updateStatusMutation.isPending}
                            >
                                Archive
                            </DropdownMenuItem>
                        )}
                        <DropdownMenuSeparator />
                        <DropdownMenuItem
                            variant="destructive"
                            onClick={() => setShowDeleteDialog(true)}
                            disabled={deleteMutation.isPending}
                        >
                            <Trash2 className="size-3.5" />
                            Delete
                        </DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>
            </div>

            <ConfirmDialog
                open={showDeleteDialog}
                onOpenChange={setShowDeleteDialog}
                title="Delete plan"
                description={`Delete plan "${plan.title}"? This cannot be undone.`}
                confirmLabel="Delete"
                confirmVariant="destructive"
                onConfirm={() => deleteMutation.mutate()}
                loading={deleteMutation.isPending}
            />
        </div>
    );
}
