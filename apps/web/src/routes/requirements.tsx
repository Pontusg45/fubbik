import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Bot, ChevronDown, ChevronRight, ClipboardCheck, FolderOpen, Pencil, Plus, Trash2, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/requirements")({
    component: RequirementsPage,
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

const STATUS_OPTIONS = [
    { value: "", label: "All statuses" },
    { value: "passing", label: "Passing" },
    { value: "failing", label: "Failing" },
    { value: "untested", label: "Untested" }
];

const PRIORITY_OPTIONS = [
    { value: "", label: "All priorities" },
    { value: "must", label: "Must" },
    { value: "should", label: "Should" },
    { value: "could", label: "Could" },
    { value: "wont", label: "Won't" }
];

function statusColor(status: string) {
    switch (status) {
        case "passing":
            return "text-green-600 bg-green-500/10 border-green-500/30";
        case "failing":
            return "text-red-600 bg-red-500/10 border-red-500/30";
        default:
            return "text-muted-foreground bg-muted";
    }
}

function priorityLabel(priority: string) {
    switch (priority) {
        case "must":
            return "Must";
        case "should":
            return "Should";
        case "could":
            return "Could";
        case "wont":
            return "Won't";
        default:
            return priority;
    }
}

interface UseCase {
    id: string;
    name: string;
    description: string | null;
    requirementCount: number;
}

function RequirementsPage() {
    const queryClient = useQueryClient();
    const { codebaseId } = useActiveCodebase();
    const [statusFilter, setStatusFilter] = useState("");
    const [priorityFilter, setPriorityFilter] = useState("");
    const [originFilter, setOriginFilter] = useState("");
    const [reviewStatusFilter, setReviewStatusFilter] = useState("");
    const [groupByUseCase, setGroupByUseCase] = useState(false);

    // Use case management state
    const [showNewUseCase, setShowNewUseCase] = useState(false);
    const [newUseCaseName, setNewUseCaseName] = useState("");
    const [newUseCaseDescription, setNewUseCaseDescription] = useState("");
    const [editingUseCaseId, setEditingUseCaseId] = useState<string | null>(null);
    const [editUseCaseName, setEditUseCaseName] = useState("");
    const [editUseCaseDescription, setEditUseCaseDescription] = useState("");

    const statsQuery = useQuery({
        queryKey: ["requirements-stats", codebaseId],
        queryFn: async () => {
            const query: { codebaseId?: string } = {};
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await api.api.requirements.stats.get({ query }));
        }
    });

    const listQuery = useQuery({
        queryKey: ["requirements", codebaseId, statusFilter, priorityFilter, originFilter, reviewStatusFilter],
        queryFn: async () => {
            const query: { codebaseId?: string; status?: string; priority?: string; origin?: "human" | "ai"; reviewStatus?: "draft" | "reviewed" | "approved"; limit?: string } = {
                limit: "50"
            };
            if (codebaseId) query.codebaseId = codebaseId;
            if (statusFilter) query.status = statusFilter;
            if (priorityFilter) query.priority = priorityFilter;
            if (originFilter) query.origin = originFilter as "human" | "ai";
            if (reviewStatusFilter) query.reviewStatus = reviewStatusFilter as "draft" | "reviewed" | "approved";
            return unwrapEden(await api.api.requirements.get({ query }));
        }
    });

    const useCasesQuery = useQuery({
        queryKey: ["use-cases", codebaseId],
        queryFn: async () => {
            const query: { codebaseId?: string } = {};
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await api.api["use-cases"].get({ query })) as UseCase[];
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => {
            return unwrapEden(await api.api.requirements({ id }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            queryClient.invalidateQueries({ queryKey: ["requirements-stats"] });
            queryClient.invalidateQueries({ queryKey: ["use-cases"] });
            toast.success("Requirement deleted");
        },
        onError: () => {
            toast.error("Failed to delete requirement");
        }
    });

    const statusMutation = useMutation({
        mutationFn: async ({ id, status }: { id: string; status: "passing" | "failing" | "untested" }) => {
            return unwrapEden(await api.api.requirements({ id }).status.patch({ status }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            queryClient.invalidateQueries({ queryKey: ["requirements-stats"] });
        },
        onError: () => {
            toast.error("Failed to update status");
        }
    });

    // Use case mutations
    const createUseCaseMutation = useMutation({
        mutationFn: async () => {
            const body: { name: string; description?: string; codebaseId?: string } = {
                name: newUseCaseName.trim()
            };
            if (newUseCaseDescription.trim()) body.description = newUseCaseDescription.trim();
            if (codebaseId) body.codebaseId = codebaseId;
            return unwrapEden(await api.api["use-cases"].post(body));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["use-cases"] });
            setShowNewUseCase(false);
            setNewUseCaseName("");
            setNewUseCaseDescription("");
            toast.success("Use case created");
        },
        onError: () => {
            toast.error("Failed to create use case");
        }
    });

    const updateUseCaseMutation = useMutation({
        mutationFn: async ({ id }: { id: string }) => {
            const body: { name?: string; description?: string | null } = {};
            if (editUseCaseName.trim()) body.name = editUseCaseName.trim();
            body.description = editUseCaseDescription.trim() || null;
            return unwrapEden(await api.api["use-cases"]({ id }).patch(body));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["use-cases"] });
            setEditingUseCaseId(null);
            toast.success("Use case updated");
        },
        onError: () => {
            toast.error("Failed to update use case");
        }
    });

    const deleteUseCaseMutation = useMutation({
        mutationFn: async (id: string) => {
            return unwrapEden(await api.api["use-cases"]({ id }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["use-cases"] });
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            toast.success("Use case deleted, requirements ungrouped");
        },
        onError: () => {
            toast.error("Failed to delete use case");
        }
    });

    const stats = statsQuery.data as { total: number; passing: number; failing: number; untested: number } | undefined;
    const data = listQuery.data as { requirements: Array<Record<string, unknown>>; total: number } | undefined;
    const requirements = data?.requirements ?? [];
    const useCases = useCasesQuery.data ?? [];
    const useCaseMap = new Map(useCases.map(uc => [uc.id, uc]));

    function handleDelete(id: string, title: string) {
        if (!confirm(`Delete requirement "${title}"?`)) return;
        deleteMutation.mutate(id);
    }

    function handleDeleteUseCase(id: string, name: string) {
        if (!confirm(`Delete use case "${name}"? Requirements will become ungrouped.`)) return;
        deleteUseCaseMutation.mutate(id);
    }

    function startEditUseCase(uc: UseCase) {
        setEditingUseCaseId(uc.id);
        setEditUseCaseName(uc.name);
        setEditUseCaseDescription(uc.description ?? "");
    }

    // Group requirements by use case
    const grouped = new Map<string | null, Array<Record<string, unknown>>>();
    for (const req of requirements) {
        const ucId = (req.useCaseId as string | null) ?? null;
        if (!grouped.has(ucId)) grouped.set(ucId, []);
        grouped.get(ucId)!.push(req);
    }

    function renderRequirementRow(req: Record<string, unknown>) {
        const id = req.id as string;
        const title = req.title as string;
        const status = (req.status as string) ?? "untested";
        const priority = req.priority as string | null;
        const steps = req.steps as Array<unknown>;
        const createdAt = req.createdAt as string;
        const reqOrigin = req.origin as string | undefined;
        const reqReviewStatus = req.reviewStatus as string | undefined;
        const reqUseCaseId = req.useCaseId as string | null | undefined;

        return (
            <div key={id} className="flex items-center justify-between gap-4 py-3 first:pt-0 last:pb-0">
                <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                        <Link
                            to="/requirements/$requirementId"
                            params={{ requirementId: id }}
                            className="truncate font-medium hover:underline"
                        >
                            {title}
                        </Link>
                        <Badge variant="outline" size="sm" className={statusColor(status)}>
                            {status}
                        </Badge>
                        {reqOrigin === "ai" && (
                            <Badge
                                variant="outline"
                                size="sm"
                                className={
                                    reqReviewStatus === "draft"
                                        ? "border-yellow-500/30 bg-yellow-500/10 text-[10px] text-yellow-600"
                                        : reqReviewStatus === "reviewed"
                                          ? "border-blue-500/30 bg-blue-500/10 text-[10px] text-blue-600"
                                          : "border-green-500/30 bg-green-500/10 text-[10px] text-green-600"
                                }
                            >
                                <Bot className="mr-0.5 size-2.5" />
                                AI
                            </Badge>
                        )}
                        {priority && (
                            <Badge variant="secondary" size="sm">
                                {priorityLabel(priority)}
                            </Badge>
                        )}
                        {!groupByUseCase && reqUseCaseId && useCaseMap.has(reqUseCaseId) && (
                            <Badge variant="outline" size="sm" className="border-indigo-500/30 bg-indigo-500/10 text-[10px] text-indigo-600">
                                <FolderOpen className="mr-0.5 size-2.5" />
                                {useCaseMap.get(reqUseCaseId)!.name}
                            </Badge>
                        )}
                    </div>
                    <div className="text-muted-foreground mt-0.5 flex items-center gap-3 text-xs">
                        <span>{steps?.length ?? 0} steps</span>
                        <span>{new Date(createdAt).toLocaleDateString()}</span>
                    </div>
                </div>
                <div className="flex items-center gap-1">
                    <select
                        value={status}
                        onChange={e => statusMutation.mutate({ id, status: e.target.value as "passing" | "failing" | "untested" })}
                        className="bg-background rounded border px-2 py-1 text-xs"
                        disabled={statusMutation.isPending}
                    >
                        <option value="passing">Passing</option>
                        <option value="failing">Failing</option>
                        <option value="untested">Untested</option>
                    </select>
                    <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleDelete(id, title)}
                        disabled={deleteMutation.isPending}
                    >
                        <Trash2 className="size-4" />
                    </Button>
                </div>
            </div>
        );
    }

    return (
        <div className="container mx-auto max-w-5xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <ClipboardCheck className="size-5" />
                    <h1 className="text-2xl font-bold tracking-tight">Requirements</h1>
                    {stats && (
                        <Badge variant="secondary" className="ml-2">
                            {stats.total}
                        </Badge>
                    )}
                </div>
                <div className="flex items-center gap-2">
                    <Button variant="outline" size="sm" onClick={() => setShowNewUseCase(true)}>
                        <FolderOpen className="mr-1 size-4" />
                        New Use Case
                    </Button>
                    <Button size="sm" render={<Link to="/requirements/new" />}>
                        <Plus className="mr-1 size-4" />
                        New Requirement
                    </Button>
                </div>
            </div>

            {/* Stats bar */}
            {stats && stats.total > 0 && (
                <div className="mb-6 grid grid-cols-4 gap-3">
                    <Card>
                        <CardPanel className="p-4 text-center">
                            <p className="text-2xl font-bold">{stats.total}</p>
                            <p className="text-muted-foreground text-xs">Total</p>
                        </CardPanel>
                    </Card>
                    <Card>
                        <CardPanel className="p-4 text-center">
                            <p className="text-2xl font-bold text-green-600">{stats.passing}</p>
                            <p className="text-muted-foreground text-xs">Passing</p>
                        </CardPanel>
                    </Card>
                    <Card>
                        <CardPanel className="p-4 text-center">
                            <p className="text-2xl font-bold text-red-600">{stats.failing}</p>
                            <p className="text-muted-foreground text-xs">Failing</p>
                        </CardPanel>
                    </Card>
                    <Card>
                        <CardPanel className="p-4 text-center">
                            <p className="text-muted-foreground text-2xl font-bold">{stats.untested}</p>
                            <p className="text-muted-foreground text-xs">Untested</p>
                        </CardPanel>
                    </Card>
                </div>
            )}

            {/* New use case inline form */}
            {showNewUseCase && (
                <Card className="mb-4">
                    <CardPanel className="p-4">
                        <div className="flex items-start gap-3">
                            <div className="flex-1 space-y-2">
                                <Input
                                    value={newUseCaseName}
                                    onChange={e => setNewUseCaseName(e.target.value)}
                                    placeholder="Use case name..."
                                    autoFocus
                                />
                                <Input
                                    value={newUseCaseDescription}
                                    onChange={e => setNewUseCaseDescription(e.target.value)}
                                    placeholder="Description (optional)..."
                                />
                            </div>
                            <div className="flex gap-1">
                                <Button
                                    size="sm"
                                    onClick={() => createUseCaseMutation.mutate()}
                                    disabled={!newUseCaseName.trim() || createUseCaseMutation.isPending}
                                >
                                    {createUseCaseMutation.isPending ? "Creating..." : "Create"}
                                </Button>
                                <Button variant="ghost" size="sm" onClick={() => setShowNewUseCase(false)}>
                                    <X className="size-4" />
                                </Button>
                            </div>
                        </div>
                    </CardPanel>
                </Card>
            )}

            {/* Filters */}
            <div className="mb-4 flex flex-wrap gap-3">
                <select
                    value={statusFilter}
                    onChange={e => setStatusFilter(e.target.value)}
                    className="bg-background focus:ring-ring rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                >
                    {STATUS_OPTIONS.map(o => (
                        <option key={o.value} value={o.value}>
                            {o.label}
                        </option>
                    ))}
                </select>
                <select
                    value={priorityFilter}
                    onChange={e => setPriorityFilter(e.target.value)}
                    className="bg-background focus:ring-ring rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                >
                    {PRIORITY_OPTIONS.map(o => (
                        <option key={o.value} value={o.value}>
                            {o.label}
                        </option>
                    ))}
                </select>
                <select
                    value={originFilter}
                    onChange={e => setOriginFilter(e.target.value)}
                    className="bg-background focus:ring-ring rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                >
                    <option value="">All origins</option>
                    <option value="human">Human</option>
                    <option value="ai">AI</option>
                </select>
                <select
                    value={reviewStatusFilter}
                    onChange={e => setReviewStatusFilter(e.target.value)}
                    className="bg-background focus:ring-ring rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                >
                    <option value="">All review statuses</option>
                    <option value="draft">Draft</option>
                    <option value="reviewed">Reviewed</option>
                    <option value="approved">Approved</option>
                </select>
                {useCases.length > 0 && (
                    <button
                        type="button"
                        onClick={() => setGroupByUseCase(!groupByUseCase)}
                        className={`flex items-center gap-1 rounded-md border px-3 py-2 text-sm transition-colors ${
                            groupByUseCase
                                ? "border-indigo-500/50 bg-indigo-500/10 text-indigo-600"
                                : "bg-background text-muted-foreground hover:text-foreground"
                        }`}
                    >
                        <FolderOpen className="size-3.5" />
                        Group by use case
                    </button>
                )}
            </div>

            {/* List */}
            {listQuery.isLoading ? (
                <Card>
                    <CardPanel className="p-6">
                        <p className="text-muted-foreground text-sm">Loading...</p>
                    </CardPanel>
                </Card>
            ) : requirements.length === 0 ? (
                <Card>
                    <CardPanel className="p-6">
                        <div className="flex flex-col items-center gap-3 py-12">
                            <ClipboardCheck className="text-muted-foreground/20 size-10" />
                            <div className="text-center">
                                <p className="text-muted-foreground font-medium">No requirements yet</p>
                                <p className="text-muted-foreground/70 mt-1 text-sm">Define requirements to track what your knowledge base should cover.</p>
                            </div>
                            <Button size="sm" render={<Link to="/requirements/new" />}>
                                <Plus className="size-3.5" />
                                Create your first requirement
                            </Button>
                        </div>
                    </CardPanel>
                </Card>
            ) : groupByUseCase ? (
                <div className="space-y-4">
                    {/* Render each use case group */}
                    {useCases.map(uc => {
                        const reqs = grouped.get(uc.id) ?? [];
                        if (reqs.length === 0) return null;

                        return (
                            <UseCaseSection
                                key={uc.id}
                                useCase={uc}
                                requirements={reqs}
                                isEditing={editingUseCaseId === uc.id}
                                editName={editUseCaseName}
                                editDescription={editUseCaseDescription}
                                onEditNameChange={setEditUseCaseName}
                                onEditDescriptionChange={setEditUseCaseDescription}
                                onStartEdit={() => startEditUseCase(uc)}
                                onSaveEdit={() => updateUseCaseMutation.mutate({ id: uc.id })}
                                onCancelEdit={() => setEditingUseCaseId(null)}
                                onDelete={() => handleDeleteUseCase(uc.id, uc.name)}
                                isSaving={updateUseCaseMutation.isPending}
                                renderRow={renderRequirementRow}
                            />
                        );
                    })}
                    {/* Ungrouped requirements */}
                    {grouped.has(null) && (grouped.get(null)?.length ?? 0) > 0 && (
                        <Card>
                            <CardPanel className="p-6">
                                <h3 className="text-muted-foreground mb-3 text-sm font-medium">Ungrouped</h3>
                                <div className="divide-y">
                                    {grouped.get(null)!.map(req => renderRequirementRow(req))}
                                </div>
                            </CardPanel>
                        </Card>
                    )}
                </div>
            ) : (
                <Card>
                    <CardPanel className="p-6">
                        <div className="divide-y">
                            {requirements.map(req => renderRequirementRow(req))}
                        </div>
                    </CardPanel>
                </Card>
            )}
        </div>
    );
}

function UseCaseSection({
    useCase,
    requirements,
    isEditing,
    editName,
    editDescription,
    onEditNameChange,
    onEditDescriptionChange,
    onStartEdit,
    onSaveEdit,
    onCancelEdit,
    onDelete,
    isSaving,
    renderRow
}: {
    useCase: UseCase;
    requirements: Array<Record<string, unknown>>;
    isEditing: boolean;
    editName: string;
    editDescription: string;
    onEditNameChange: (v: string) => void;
    onEditDescriptionChange: (v: string) => void;
    onStartEdit: () => void;
    onSaveEdit: () => void;
    onCancelEdit: () => void;
    onDelete: () => void;
    isSaving: boolean;
    renderRow: (req: Record<string, unknown>) => React.ReactNode;
}) {
    const [collapsed, setCollapsed] = useState(false);

    return (
        <Card>
            <CardPanel className="p-6">
                {isEditing ? (
                    <div className="mb-3 flex items-start gap-3">
                        <div className="flex-1 space-y-2">
                            <Input value={editName} onChange={e => onEditNameChange(e.target.value)} autoFocus />
                            <Input
                                value={editDescription}
                                onChange={e => onEditDescriptionChange(e.target.value)}
                                placeholder="Description (optional)..."
                            />
                        </div>
                        <div className="flex gap-1">
                            <Button size="sm" onClick={onSaveEdit} disabled={!editName.trim() || isSaving}>
                                {isSaving ? "Saving..." : "Save"}
                            </Button>
                            <Button variant="ghost" size="sm" onClick={onCancelEdit}>
                                <X className="size-4" />
                            </Button>
                        </div>
                    </div>
                ) : (
                    <div className="group mb-3 flex items-center gap-2">
                        <button type="button" onClick={() => setCollapsed(!collapsed)} className="flex items-center gap-1">
                            {collapsed ? <ChevronRight className="size-4" /> : <ChevronDown className="size-4" />}
                        </button>
                        <FolderOpen className="size-4 text-indigo-500" />
                        <h3 className="font-medium">{useCase.name}</h3>
                        <Badge variant="secondary" size="sm">
                            {requirements.length}
                        </Badge>
                        {useCase.description && (
                            <span className="text-muted-foreground text-xs">{useCase.description}</span>
                        )}
                        <div className="ml-auto flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                            <Button variant="ghost" size="sm" onClick={onStartEdit}>
                                <Pencil className="size-3.5" />
                            </Button>
                            <Button variant="ghost" size="sm" onClick={onDelete}>
                                <Trash2 className="size-3.5" />
                            </Button>
                        </div>
                    </div>
                )}
                {!collapsed && (
                    <div className="divide-y">
                        {requirements.map(req => renderRow(req))}
                    </div>
                )}
            </CardPanel>
        </Card>
    );
}
