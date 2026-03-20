import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/plans/new")({
    component: NewPlan,
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            return { session: null };
        }
    }
});

interface StepRow {
    description: string;
}

function NewPlan() {
    const navigate = useNavigate();
    const [title, setTitle] = useState("");
    const [description, setDescription] = useState("");
    const [steps, setSteps] = useState<StepRow[]>([{ description: "" }]);

    const createMutation = useMutation({
        mutationFn: async () => {
            const validSteps = steps
                .filter(s => s.description.trim())
                .map((s, i) => ({ description: s.description.trim(), order: i }));

            return unwrapEden(
                await api.api.plans.post({
                    title: title.trim(),
                    ...(description.trim() ? { description: description.trim() } : {}),
                    ...(validSteps.length > 0 ? { steps: validSteps } : {})
                })
            );
        },
        onSuccess: (data) => {
            toast.success("Plan created");
            const planId = (data as Record<string, unknown>)?.id as string | undefined;
            if (planId) {
                navigate({ to: "/plans/$planId", params: { planId } });
            } else {
                navigate({ to: "/plans" });
            }
        },
        onError: () => {
            toast.error("Failed to create plan");
        }
    });

    function addStepAfter(index: number) {
        const next = [...steps];
        next.splice(index + 1, 0, { description: "" });
        setSteps(next);
    }

    function removeStep(index: number) {
        if (steps.length <= 1) return;
        setSteps(steps.filter((_, i) => i !== index));
    }

    function updateStep(index: number, value: string) {
        setSteps(steps.map((s, i) => (i === index ? { description: value } : s)));
    }

    function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        if (!title.trim()) return;
        createMutation.mutate();
    }

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6">
                <Button variant="ghost" size="sm" render={<Link to="/plans" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
            </div>

            <h1 className="mb-6 text-2xl font-bold tracking-tight">New Plan</h1>

            <Card>
                <CardPanel className="p-6">
                    <form onSubmit={handleSubmit} className="space-y-4">
                        <div>
                            <label className="mb-1.5 block text-sm font-medium">Title</label>
                            <input
                                type="text"
                                value={title}
                                onChange={e => setTitle(e.target.value)}
                                placeholder="Enter a plan title..."
                                required
                                className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                            />
                        </div>

                        <div>
                            <label className="mb-1.5 block text-sm font-medium">Description</label>
                            <textarea
                                value={description}
                                onChange={e => setDescription(e.target.value)}
                                placeholder="Describe what this plan covers (optional)..."
                                rows={3}
                                className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                            />
                        </div>

                        <Separator />

                        <div>
                            <label className="mb-2 block text-sm font-medium">Steps</label>
                            <div className="space-y-2">
                                {steps.map((step, i) => (
                                    <div key={i} className="flex items-center gap-2">
                                        <span className="text-muted-foreground w-6 text-right text-xs font-mono">
                                            {i + 1}.
                                        </span>
                                        <input
                                            type="text"
                                            value={step.description}
                                            onChange={e => updateStep(i, e.target.value)}
                                            placeholder="Step description..."
                                            className="bg-background focus:ring-ring flex-1 rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                            onKeyDown={e => {
                                                if (e.key === "Enter") {
                                                    e.preventDefault();
                                                    addStepAfter(i);
                                                }
                                            }}
                                        />
                                        <Button
                                            type="button"
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => addStepAfter(i)}
                                            title="Add step below"
                                            className="size-8 p-0"
                                        >
                                            <Plus className="size-3.5" />
                                        </Button>
                                        <Button
                                            type="button"
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => removeStep(i)}
                                            disabled={steps.length <= 1}
                                            title="Remove step"
                                            className="size-8 p-0"
                                        >
                                            <Trash2 className="size-3.5" />
                                        </Button>
                                    </div>
                                ))}
                            </div>
                        </div>

                        <Separator />

                        <div className="flex justify-end gap-2">
                            <Button type="button" variant="outline" render={<Link to="/plans" />}>
                                Cancel
                            </Button>
                            <Button
                                type="submit"
                                disabled={createMutation.isPending || !title.trim()}
                            >
                                {createMutation.isPending ? "Creating..." : "Create Plan"}
                            </Button>
                        </div>
                    </form>
                </CardPanel>
            </Card>
        </div>
    );
}
