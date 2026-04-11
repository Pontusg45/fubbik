import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PageContainer, PageHeader } from "@/components/ui/page";
import { Textarea } from "@/components/ui/textarea";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/plans/new")({ component: NewPlanPage });

function NewPlanPage() {
    const navigate = useNavigate();
    const [title, setTitle] = useState("");
    const [description, setDescription] = useState("");
    const [codebaseId, setCodebaseId] = useState<string>("");

    const codebasesQuery = useQuery({
        queryKey: ["codebases"],
        queryFn: async () => {
            try {
                return (unwrapEden(await api.api.codebases.get()) as any[]) ?? [];
            } catch {
                return [];
            }
        },
    });

    const createMutation = useMutation({
        mutationFn: async () => {
            const body: any = { title: title.trim() };
            if (description.trim()) body.description = description.trim();
            if (codebaseId) body.codebaseId = codebaseId;
            return unwrapEden(await api.api.plans.post(body));
        },
        onSuccess: plan => {
            navigate({ to: "/plans/$planId", params: { planId: (plan as any).id } });
        },
    });

    return (
        <PageContainer>
            <PageHeader
                title="New Plan"
                description="Start with a title and description. Link requirements, add analyze notes, and draft tasks on the detail page."
            />
            <form
                onSubmit={e => {
                    e.preventDefault();
                    if (!title.trim()) return;
                    createMutation.mutate();
                }}
                className="max-w-xl space-y-4"
            >
                <div className="space-y-2">
                    <Label htmlFor="title">Title</Label>
                    <Input
                        id="title"
                        value={title}
                        onChange={e => setTitle(e.target.value)}
                        required
                        autoFocus
                    />
                </div>
                <div className="space-y-2">
                    <Label htmlFor="description">Description (markdown)</Label>
                    <Textarea
                        id="description"
                        value={description}
                        onChange={e => setDescription(e.target.value)}
                        rows={6}
                        placeholder="What is this plan about?"
                    />
                </div>
                <div className="space-y-2">
                    <Label htmlFor="codebase">Codebase (optional)</Label>
                    <select
                        id="codebase"
                        value={codebaseId}
                        onChange={e => setCodebaseId(e.target.value)}
                        className="w-full rounded-md border bg-background px-3 py-2 text-sm"
                    >
                        <option value="">— none —</option>
                        {(codebasesQuery.data ?? []).map((c: any) => (
                            <option key={c.id} value={c.id}>
                                {c.name}
                            </option>
                        ))}
                    </select>
                </div>
                <div className="flex gap-2">
                    <Button type="submit" disabled={!title.trim() || createMutation.isPending}>
                        {createMutation.isPending ? "Creating…" : "Create Plan"}
                    </Button>
                    <Button
                        type="button"
                        variant="ghost"
                        onClick={() => navigate({ to: "/plans" })}
                    >
                        Cancel
                    </Button>
                </div>
            </form>
        </PageContainer>
    );
}
