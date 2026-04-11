import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { Bot, Check, Copy, Pencil, Trash2, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { BackLink } from "@/components/back-link";
import { ConfirmDialog } from "@/components/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ChunkLinker } from "@/features/requirements/chunk-linker";
import { DependencyGraph } from "@/features/requirements/dependency-graph";
import { DependencySection } from "@/features/requirements/dependency-section";
import { RequirementPlans } from "@/features/requirements/requirement-plans";
import { StepBuilder } from "@/features/requirements/step-builder";
import { validateSteps, type Keyword, type StepRow, type StepError } from "@/features/requirements/validation";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/requirements_/$requirementId")({
    component: RequirementDetail,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

type Status = "passing" | "failing" | "untested";

const STATUS_STYLES: Record<string, string> = {
    passing: "bg-emerald-500/10 text-emerald-600 border-emerald-500/30 dark:text-emerald-400",
    failing: "bg-red-500/10 text-red-600 border-red-500/30 dark:text-red-400",
    untested: "bg-muted text-muted-foreground"
};

const KEYWORD_STYLES: Record<string, string> = {
    given: "text-blue-600 dark:text-blue-400",
    when: "text-amber-600 dark:text-amber-400",
    then: "text-emerald-600 dark:text-emerald-400",
    and: "text-muted-foreground",
    but: "text-muted-foreground"
};

const PRIORITY_LABELS: Record<string, string> = {
    must: "Must",
    should: "Should",
    could: "Could",
    wont: "Won't"
};

function RequirementDetail() {
    const { requirementId } = Route.useParams();
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const [showDeleteDialog, setShowDeleteDialog] = useState(false);

    // Edit mode state
    const [editing, setEditing] = useState(false);
    const [editTitle, setEditTitle] = useState("");
    const [editDescription, setEditDescription] = useState("");
    const [editPriority, setEditPriority] = useState("");
    const [editUseCaseId, setEditUseCaseId] = useState("");
    const [editSteps, setEditSteps] = useState<StepRow[]>([]);
    const [editChunkIds, setEditChunkIds] = useState<string[]>([]);
    const [editStepErrors, setEditStepErrors] = useState<StepError[]>([]);

    const { codebaseId } = useActiveCodebase();

    const { data, isLoading, error } = useQuery({
        queryKey: ["requirement", requirementId],
        queryFn: async () => {
            const res = await api.api.requirements({ id: requirementId }).get();
            if (res.error) throw new Error("Failed to fetch");
            return res.data as Record<string, unknown>;
        }
    });

    const useCasesQuery = useQuery({
        queryKey: ["use-cases", codebaseId],
        queryFn: async () => {
            const query: { codebaseId?: string } = {};
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await api.api["use-cases"].get({ query })) as Array<{ id: string; name: string }>;
        },
        enabled: editing
    });

    const statusMutation = useMutation({
        mutationFn: async (status: Status) => {
            return unwrapEden(await api.api.requirements({ id: requirementId }).status.patch({ status }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirement", requirementId] });
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            toast.success("Status updated");
        },
        onError: () => toast.error("Failed to update status")
    });

    const reviewMutation = useMutation({
        mutationFn: async (reviewStatus: "reviewed" | "approved") => {
            return unwrapEden(await api.api.requirements({ id: requirementId }).patch({ reviewStatus }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirement", requirementId] });
            toast.success("Review status updated");
        },
        onError: () => toast.error("Failed to update review status")
    });

    const deleteMutation = useMutation({
        mutationFn: async () => {
            return unwrapEden(await api.api.requirements({ id: requirementId }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            toast.success("Deleted");
            navigate({ to: "/requirements" });
        },
        onError: () => toast.error("Failed to delete")
    });

    const updateMutation = useMutation({
        mutationFn: async () => {
            const body: Record<string, unknown> = {};
            if (editTitle.trim() !== title) body.title = editTitle.trim();
            if (editDescription.trim() !== (description ?? "")) body.description = editDescription.trim() || null;
            if (editPriority !== (priority ?? "")) body.priority = editPriority || null;
            if (editUseCaseId !== ((data!.useCaseId as string) ?? "")) body.useCaseId = editUseCaseId || null;

            const stepsChanged = JSON.stringify(editSteps) !== JSON.stringify(steps);
            if (stepsChanged) body.steps = editSteps.map(s => ({ keyword: s.keyword, text: s.text.trim() }));

            if (Object.keys(body).length > 0) {
                await unwrapEden(await api.api.requirements({ id: requirementId }).patch(body));
            }

            const chunksChanged = JSON.stringify([...editChunkIds].sort()) !== JSON.stringify(chunks.map(c => c.id).sort());
            if (chunksChanged) {
                await unwrapEden(await api.api.requirements({ id: requirementId }).chunks.put({ chunkIds: editChunkIds }));
            }
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirement", requirementId] });
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            setEditing(false);
            toast.success("Requirement updated");
        },
        onError: () => toast.error("Failed to update")
    });

    function enterEditMode() {
        setEditTitle(title);
        setEditDescription(description ?? "");
        setEditPriority(priority ?? "");
        setEditUseCaseId((data!.useCaseId as string) ?? "");
        setEditSteps(steps.map(s => ({ keyword: s.keyword as Keyword, text: s.text })));
        setEditChunkIds(chunks.map(c => c.id));
        setEditStepErrors([]);
        setEditing(true);
    }

    function handleSave() {
        const errors = validateSteps(editSteps);
        editSteps.forEach((s, i) => {
            if (!s.text.trim()) errors.push({ step: i, error: "Step text is required" });
        });
        setEditStepErrors(errors);
        if (errors.length === 0 && editTitle.trim()) {
            updateMutation.mutate();
        }
    }

    async function handleExport(format: "gherkin" | "vitest" | "markdown") {
        try {
            const result = unwrapEden(
                await api.api.requirements({ id: requirementId }).export.get({ query: { format } })
            );
            const text = typeof result === "string" ? result : JSON.stringify(result, null, 2);
            await navigator.clipboard.writeText(text);
            toast.success(`${format.charAt(0).toUpperCase() + format.slice(1)} copied to clipboard`);
        } catch {
            toast.error("Failed to export");
        }
    }

    // Loading / error states
    if (isLoading) {
        return (
            <Shell>
                <p className="text-muted-foreground py-12 text-center text-sm">Loading...</p>
            </Shell>
        );
    }

    if (error || !data) {
        return (
            <Shell>
                <p className="text-muted-foreground py-12 text-center text-sm">Requirement not found.</p>
            </Shell>
        );
    }

    const title = (data.title as string) ?? "Untitled";
    const description = data.description as string | null;
    const status = ((data.status as string) ?? "untested") as Status;
    const priority = data.priority as string | null;
    const origin = data.origin as string | undefined;
    const reviewStatus = (data.reviewStatus as string) ?? "approved";
    const isAi = origin === "ai";
    const steps = (data.steps as Array<{ keyword: string; text: string }>) ?? [];
    const chunks = (data.chunks as Array<{ id: string; title: string }>) ?? [];
    const warnings = (data.warnings as Array<{ step: number; warning: string }>) ?? [];
    const vocabWarnings = (data.vocabularyWarnings as Array<{ step: number; type: string; word: string; message: string }>) ?? [];

    return (
        <Shell>
            {/* Header */}
            <div className="mb-6 flex items-start justify-between">
                <div>
                    {editing ? (
                        <Input
                            value={editTitle}
                            onChange={e => setEditTitle(e.target.value)}
                            className="text-2xl font-bold"
                            placeholder="Requirement title..."
                        />
                    ) : (
                        <h1 className="text-2xl font-bold tracking-tight">{title}</h1>
                    )}
                    {!editing && (
                        <div className="mt-2 flex flex-wrap items-center gap-2">
                            <Badge variant="outline" className={STATUS_STYLES[status] ?? STATUS_STYLES.untested}>
                                {status}
                            </Badge>
                            {priority && (
                                <Badge variant="secondary">{PRIORITY_LABELS[priority] ?? priority}</Badge>
                            )}
                            {isAi && (
                                <Badge
                                    variant="outline"
                                    className={
                                        reviewStatus === "draft"
                                            ? "border-yellow-500/30 bg-yellow-500/10 text-yellow-600 dark:text-yellow-400"
                                            : reviewStatus === "reviewed"
                                              ? "border-blue-500/30 bg-blue-500/10 text-blue-600 dark:text-blue-400"
                                              : "border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
                                    }
                                >
                                    <Bot className="mr-1 size-3" />
                                    AI {reviewStatus === "draft" ? "Draft" : reviewStatus === "reviewed" ? "Reviewed" : "Approved"}
                                </Badge>
                            )}
                        </div>
                    )}
                    {!editing && description && (
                        <p className="text-muted-foreground mt-3 text-sm leading-relaxed">{description}</p>
                    )}
                </div>
                {!editing && (
                    <Button variant="outline" size="sm" onClick={enterEditMode}>
                        <Pencil className="mr-1 size-3.5" />
                        Edit
                    </Button>
                )}
            </div>

            {editing ? (
                <div className="space-y-4">
                    {/* Description textarea */}
                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Description</label>
                        <textarea
                            value={editDescription}
                            onChange={e => setEditDescription(e.target.value)}
                            className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                            rows={3}
                            placeholder="Describe the requirement..."
                        />
                    </div>
                    {/* Priority + Use Case */}
                    <div className="flex gap-4">
                        <div className="flex-1">
                            <label className="mb-1.5 block text-sm font-medium">Priority</label>
                            <select
                                value={editPriority}
                                onChange={e => setEditPriority(e.target.value)}
                                className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                            >
                                <option value="">None</option>
                                <option value="must">Must</option>
                                <option value="should">Should</option>
                                <option value="could">Could</option>
                                <option value="wont">Won't</option>
                            </select>
                        </div>
                        <div className="flex-1">
                            <label className="mb-1.5 block text-sm font-medium">Use Case</label>
                            <select
                                value={editUseCaseId}
                                onChange={e => setEditUseCaseId(e.target.value)}
                                className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                            >
                                <option value="">(none)</option>
                                {(useCasesQuery.data ?? []).map(uc => (
                                    <option key={uc.id} value={uc.id}>{uc.name}</option>
                                ))}
                            </select>
                        </div>
                    </div>
                    {/* Step builder */}
                    <StepBuilder steps={editSteps} onStepsChange={setEditSteps} codebaseId={codebaseId} stepErrors={editStepErrors} />
                    {/* Chunk linker */}
                    <ChunkLinker selectedChunkIds={editChunkIds} onSelectedChunkIdsChange={setEditChunkIds} codebaseId={codebaseId} />
                    {/* Save/Cancel */}
                    <div className="flex justify-end gap-2 border-t pt-4">
                        <Button variant="outline" onClick={() => setEditing(false)}>Cancel</Button>
                        <Button onClick={handleSave} disabled={updateMutation.isPending}>
                            {updateMutation.isPending ? "Saving..." : "Save Changes"}
                        </Button>
                    </div>
                </div>
            ) : (
                <>
                    {/* Status + Review controls */}
                    <div className="mb-6 flex flex-wrap items-center gap-3 rounded-lg border p-3">
                        <span className="text-muted-foreground text-xs font-medium uppercase tracking-wide">Status</span>
                        <div className="flex gap-1">
                            {(["passing", "failing", "untested"] as const).map(s => (
                                <button
                                    key={s}
                                    onClick={() => statusMutation.mutate(s)}
                                    disabled={statusMutation.isPending}
                                    className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
                                        status === s
                                            ? s === "passing"
                                                ? "bg-emerald-500/90 text-white dark:bg-emerald-600"
                                                : s === "failing"
                                                  ? "bg-red-500/90 text-white dark:bg-red-600"
                                                  : "bg-muted text-foreground"
                                            : "text-muted-foreground hover:bg-muted/50"
                                    }`}
                                >
                                    {s === "passing" && <Check className="mr-1 inline size-3" />}
                                    {s === "failing" && <X className="mr-1 inline size-3" />}
                                    {s.charAt(0).toUpperCase() + s.slice(1)}
                                </button>
                            ))}
                        </div>

                        {isAi && reviewStatus !== "approved" && (
                            <>
                                <div className="bg-border h-6 w-px" />
                                <span className="text-muted-foreground text-xs font-medium uppercase tracking-wide">Review</span>
                                <div className="flex gap-1">
                                    {reviewStatus === "draft" && (
                                        <Button variant="outline" size="sm" onClick={() => reviewMutation.mutate("reviewed")} disabled={reviewMutation.isPending}>
                                            Mark Reviewed
                                        </Button>
                                    )}
                                    <Button variant="outline" size="sm" onClick={() => reviewMutation.mutate("approved")} disabled={reviewMutation.isPending}>
                                        Approve
                                    </Button>
                                </div>
                            </>
                        )}
                    </div>

                    {/* Steps */}
                    <div className="mb-6">
                        <h2 className="mb-3 text-sm font-semibold">Steps</h2>
                        <div className="rounded-lg border">
                            {steps.map((step, i) => (
                                <div
                                    key={i}
                                    className={`flex gap-3 px-4 py-2.5 ${i > 0 ? "border-t" : ""}`}
                                >
                                    <span className={`w-14 shrink-0 text-right font-mono text-xs font-bold uppercase ${KEYWORD_STYLES[step.keyword] ?? ""}`}>
                                        {step.keyword}
                                    </span>
                                    <span className="text-sm">{step.text}</span>
                                </div>
                            ))}
                        </div>
                    </div>

                    {/* Dependencies */}
                    <div className="mb-6">
                        <DependencySection requirementId={requirementId} />
                    </div>
                    <DependencyGraph requirementId={requirementId} />

                    {/* Warnings */}
                    {warnings.length > 0 && (
                        <div className="mb-6 rounded-lg border border-yellow-500/30 bg-yellow-500/5 p-4">
                            <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-yellow-600 dark:text-yellow-400">
                                Cross-reference Warnings
                            </h3>
                            {warnings.map((w, i) => (
                                <p key={i} className="text-sm text-yellow-700 dark:text-yellow-300">
                                    {w.step >= 0 ? `Step ${w.step + 1}: ` : ""}{w.warning}
                                </p>
                            ))}
                        </div>
                    )}

                    {vocabWarnings.length > 0 && (
                        <div className="mb-6 rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
                            <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-amber-600 dark:text-amber-400">
                                Vocabulary Warnings
                            </h3>
                            {vocabWarnings.map((w, i) => (
                                <p key={i} className="text-sm text-amber-700 dark:text-amber-300">
                                    Step {w.step + 1}: {w.message}
                                </p>
                            ))}
                        </div>
                    )}

                    {/* Linked Chunks */}
                    {chunks.length > 0 && (
                        <div className="mb-6">
                            <h2 className="mb-3 text-sm font-semibold">Linked Chunks</h2>
                            <div className="space-y-1">
                                {chunks.map(c => (
                                    <Link
                                        key={c.id}
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: c.id }}
                                        className="hover:bg-muted/50 block rounded-md border px-3 py-2 text-sm transition-colors"
                                    >
                                        {c.title}
                                    </Link>
                                ))}
                            </div>
                        </div>
                    )}

                    {/* Linked Plans */}
                    <RequirementPlans requirementId={requirementId} />

                    {/* Export */}
                    <div className="mb-6">
                        <h2 className="mb-3 text-sm font-semibold">Export</h2>
                        <div className="flex gap-2">
                            {(["gherkin", "vitest", "markdown"] as const).map(fmt => (
                                <Button key={fmt} variant="outline" size="sm" onClick={() => handleExport(fmt)}>
                                    <Copy className="size-3.5" />
                                    {fmt.charAt(0).toUpperCase() + fmt.slice(1)}
                                </Button>
                            ))}
                        </div>
                    </div>

                    {/* Delete */}
                    <div className="border-t pt-6">
                        <Button
                            variant="outline"
                            size="sm"
                            className="text-destructive"
                            onClick={() => setShowDeleteDialog(true)}
                            disabled={deleteMutation.isPending}
                        >
                            <Trash2 className="size-3.5" />
                            {deleteMutation.isPending ? "Deleting..." : "Delete Requirement"}
                        </Button>
                        <ConfirmDialog
                            open={showDeleteDialog}
                            onOpenChange={setShowDeleteDialog}
                            title="Delete requirement"
                            description={`Delete "${title}"? This cannot be undone.`}
                            confirmLabel="Delete"
                            confirmVariant="destructive"
                            onConfirm={() => {
                                setShowDeleteDialog(false);
                                deleteMutation.mutate();
                            }}
                            loading={deleteMutation.isPending}
                        />
                    </div>
                </>
            )}
        </Shell>
    );
}

function Shell({ children }: { children: React.ReactNode }) {
    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <BackLink to="/requirements" label="Requirements" />
            {children}
        </div>
    );
}
