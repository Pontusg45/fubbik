import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useMemo, useState } from "react";

import { MarkdownRenderer } from "@/components/markdown-renderer";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PageContainer, PageHeader } from "@/components/ui/page";
import { Textarea } from "@/components/ui/textarea";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface RequirementRow {
    id: string;
    title: string;
    status: string;
    priority?: string | null;
}

export const Route = createFileRoute("/plans/new")({ component: NewPlanPage });

function NewPlanPage() {
    const navigate = useNavigate();
    const [title, setTitle] = useState("");
    const [description, setDescription] = useState("");
    const [descTab, setDescTab] = useState<"edit" | "preview">("edit");
    const [codebaseId, setCodebaseId] = useState<string>("");
    const [selectedRequirementIds, setSelectedRequirementIds] = useState<string[]>([]);
    const [requirementsExpanded, setRequirementsExpanded] = useState(false);
    const [requirementsQuery, setRequirementsQueryDraft] = useState("");
    const [bootstrapTasks, setBootstrapTasks] = useState("");
    const [tasksExpanded, setTasksExpanded] = useState(false);

    const codebasesQuery = useQuery({
        queryKey: ["codebases"],
        queryFn: async () => {
            try {
                return (unwrapEden(await api.api.codebases.get()) as Array<{ id: string; name: string }>) ?? [];
            } catch {
                return [];
            }
        },
    });

    const requirementsListQuery = useQuery({
        queryKey: ["requirements-for-plan-picker"],
        queryFn: async () => {
            try {
                const result = unwrapEden(await api.api.requirements.get({ query: {} }));
                // The endpoint may return either { requirements, total } or a bare array.
                const arr =
                    Array.isArray(result)
                        ? result
                        : (result as { requirements?: RequirementRow[] })?.requirements ?? [];
                return arr as RequirementRow[];
            } catch {
                return [];
            }
        },
        enabled: requirementsExpanded,
    });

    const filteredRequirements = useMemo(() => {
        const all = requirementsListQuery.data ?? [];
        const q = requirementsQuery.trim().toLowerCase();
        if (!q) return all;
        return all.filter(r => r.title.toLowerCase().includes(q));
    }, [requirementsListQuery.data, requirementsQuery]);

    const createMutation = useMutation({
        mutationFn: async () => {
            const body: any = { title: title.trim() };
            if (description.trim()) body.description = description.trim();
            if (codebaseId) body.codebaseId = codebaseId;
            if (selectedRequirementIds.length > 0) body.requirementIds = selectedRequirementIds;
            const tasks = bootstrapTasks
                .split("\n")
                .map(line => line.trim())
                .filter(Boolean)
                .map(t => ({ title: t }));
            if (tasks.length > 0) body.tasks = tasks;
            return unwrapEden(await api.api.plans.post(body));
        },
        onSuccess: plan => {
            navigate({ to: "/plans/$planId", params: { planId: (plan as any).id } });
        },
    });

    const toggleRequirement = (id: string) => {
        setSelectedRequirementIds(prev =>
            prev.includes(id) ? prev.filter(x => x !== id) : [...prev, id]
        );
    };

    const taskLineCount = bootstrapTasks
        .split("\n")
        .map(line => line.trim())
        .filter(Boolean).length;

    return (
        <PageContainer>
            <PageHeader
                title="New Plan"
                description="Start with a title and description. Optionally pre-link requirements and seed initial tasks."
            />
            <form
                onSubmit={e => {
                    e.preventDefault();
                    if (!title.trim()) return;
                    createMutation.mutate();
                }}
                className="max-w-2xl space-y-4"
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
                    <div className="flex items-center justify-between">
                        <Label htmlFor="description">Description (markdown)</Label>
                        <div className="flex gap-1 text-xs">
                            <button
                                type="button"
                                onClick={() => setDescTab("edit")}
                                className={`rounded px-2 py-0.5 ${descTab === "edit" ? "bg-muted" : "text-muted-foreground"}`}
                            >
                                Edit
                            </button>
                            <button
                                type="button"
                                onClick={() => setDescTab("preview")}
                                className={`rounded px-2 py-0.5 ${descTab === "preview" ? "bg-muted" : "text-muted-foreground"}`}
                                disabled={!description.trim()}
                            >
                                Preview
                            </button>
                        </div>
                    </div>
                    {descTab === "edit" ? (
                        <Textarea
                            id="description"
                            value={description}
                            onChange={e => setDescription(e.target.value)}
                            rows={6}
                            placeholder="What is this plan about?"
                        />
                    ) : (
                        <div className="prose prose-sm dark:prose-invert max-w-none rounded-md border bg-muted/20 p-3">
                            <MarkdownRenderer>{description || "_Nothing to preview yet._"}</MarkdownRenderer>
                        </div>
                    )}
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
                        {(codebasesQuery.data ?? []).map(c => (
                            <option key={c.id} value={c.id}>{c.name}</option>
                        ))}
                    </select>
                </div>

                {/* Requirements picker — collapsed by default */}
                <div className="rounded-md border">
                    <button
                        type="button"
                        onClick={() => setRequirementsExpanded(e => !e)}
                        className="flex w-full items-center justify-between px-3 py-2 text-sm font-medium hover:bg-muted/40"
                    >
                        <span>
                            Link requirements
                            {selectedRequirementIds.length > 0 && (
                                <span className="text-muted-foreground ml-2 text-xs font-normal">
                                    {selectedRequirementIds.length} selected
                                </span>
                            )}
                        </span>
                        <span className="text-muted-foreground text-xs">{requirementsExpanded ? "−" : "+"}</span>
                    </button>
                    {requirementsExpanded && (
                        <div className="space-y-2 border-t p-3">
                            <Input
                                placeholder="Search requirements..."
                                value={requirementsQuery}
                                onChange={e => setRequirementsQueryDraft(e.target.value)}
                                className="h-8 text-sm"
                            />
                            <div className="max-h-[240px] space-y-0.5 overflow-y-auto">
                                {requirementsListQuery.isLoading && (
                                    <p className="text-muted-foreground p-2 text-xs">Loading…</p>
                                )}
                                {!requirementsListQuery.isLoading && filteredRequirements.length === 0 && (
                                    <p className="text-muted-foreground p-2 text-xs">No requirements match.</p>
                                )}
                                {filteredRequirements.map(r => (
                                    <label
                                        key={r.id}
                                        className="hover:bg-muted/40 flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm"
                                    >
                                        <input
                                            type="checkbox"
                                            checked={selectedRequirementIds.includes(r.id)}
                                            onChange={() => toggleRequirement(r.id)}
                                        />
                                        <span className="flex-1">{r.title}</span>
                                        <span className="text-muted-foreground text-[10px] uppercase">{r.status}</span>
                                    </label>
                                ))}
                            </div>
                        </div>
                    )}
                </div>

                {/* Bootstrap tasks */}
                <div className="rounded-md border">
                    <button
                        type="button"
                        onClick={() => setTasksExpanded(e => !e)}
                        className="flex w-full items-center justify-between px-3 py-2 text-sm font-medium hover:bg-muted/40"
                    >
                        <span>
                            Seed initial tasks
                            {taskLineCount > 0 && (
                                <span className="text-muted-foreground ml-2 text-xs font-normal">
                                    {taskLineCount} task{taskLineCount === 1 ? "" : "s"}
                                </span>
                            )}
                        </span>
                        <span className="text-muted-foreground text-xs">{tasksExpanded ? "−" : "+"}</span>
                    </button>
                    {tasksExpanded && (
                        <div className="border-t p-3">
                            <Textarea
                                value={bootstrapTasks}
                                onChange={e => setBootstrapTasks(e.target.value)}
                                rows={5}
                                placeholder={"One task title per line\nE.g. Set up DB schema\nAdd seed data"}
                                className="font-mono text-xs"
                            />
                            <p className="text-muted-foreground mt-1 text-[11px]">
                                Each non-empty line becomes a task with status <code>pending</code>.
                            </p>
                        </div>
                    )}
                </div>

                <div className="flex gap-2 pt-2">
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
