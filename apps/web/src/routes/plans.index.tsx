import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate, useSearch } from "@tanstack/react-router";
import { Archive, ArchiveRestore, Copy, MoreHorizontal, Plus, Search, Trash2, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { PageContainer, PageHeader, PageLoading } from "@/components/ui/page";
import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { PlanStatusPill, type PlanStatusValue } from "@/features/plans/plan-status-pill";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

type StatusFilter = "active" | "draft" | "ready" | "in_progress" | "completed" | "archived" | "all";

const ACTIVE_STATUSES: PlanStatusValue[] = ["analyzing", "ready", "in_progress"];

interface PlanRow {
    id: string;
    title: string;
    description: string | null;
    status: PlanStatusValue;
    codebaseId: string | null;
    codebaseName: string | null;
    taskTotal: number;
    taskDone: number;
    nextAction: string | null;
    lastActivityAt: string;
    updatedAt: string;
    metadata?: Record<string, unknown>;
}

interface Codebase {
    id: string;
    name: string;
}

export const Route = createFileRoute("/plans/")({
    component: PlansIndexPage,
    validateSearch: (search): { status?: StatusFilter; codebase?: string; q?: string } => ({
        status: (search.status as StatusFilter) ?? undefined,
        codebase: (search.codebase as string) ?? undefined,
        q: (search.q as string) ?? undefined,
    }),
});

function PlansIndexPage() {
    const queryClient = useQueryClient();
    const navigate = useNavigate();
    const search = useSearch({ from: "/plans/" });

    const statusFilter: StatusFilter = search.status ?? "active";
    const codebaseId = search.codebase ?? "";
    const q = search.q ?? "";
    const [qDraft, setQDraft] = useState(q);
    const searchRef = useRef<HTMLInputElement>(null);

    const setSearch = (patch: Partial<{ status?: StatusFilter; codebase?: string; q?: string }>) => {
        navigate({
            to: "/plans",
            search: (prev: Record<string, unknown>) => {
                const next = { ...prev, ...patch };
                // Strip empties so the URL stays clean.
                for (const k of Object.keys(next) as Array<keyof typeof next>) {
                    if (next[k] === "" || next[k] === undefined) delete next[k];
                }
                return next;
            },
            replace: true,
        });
    };

    // Debounced commit of the search input → URL.
    useEffect(() => {
        const t = setTimeout(() => {
            if (qDraft !== q) setSearch({ q: qDraft || undefined });
        }, 200);
        return () => clearTimeout(t);
    }, [qDraft, q]);

    // `/` to focus search.
    useEffect(() => {
        function onKey(e: KeyboardEvent) {
            const target = e.target as HTMLElement | null;
            if (target?.tagName === "INPUT" || target?.tagName === "TEXTAREA") return;
            if (e.key === "/" && !e.metaKey && !e.ctrlKey) {
                e.preventDefault();
                searchRef.current?.focus();
            }
        }
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, []);

    const codebasesQuery = useQuery({
        queryKey: ["codebases"],
        queryFn: async () => {
            try {
                return (unwrapEden(await api.api.codebases.get()) as Codebase[]) ?? [];
            } catch {
                return [];
            }
        },
        staleTime: 60_000,
    });

    const plansQuery = useQuery({
        queryKey: ["plans", statusFilter, codebaseId],
        queryFn: async () => {
            const query: Record<string, string> = {};
            if (statusFilter === "archived") {
                query.status = "archived";
            } else if (statusFilter === "active") {
                // No server-side filter — we fetch all non-archived and narrow client-side.
                query.includeArchived = "false";
            } else if (statusFilter === "all") {
                query.includeArchived = "true";
            } else {
                query.status = statusFilter;
            }
            if (codebaseId) query.codebaseId = codebaseId;
            const result = unwrapEden(await api.api.plans.get({ query }));
            return (result as unknown as PlanRow[]) ?? [];
        },
    });

    const filteredPlans = useMemo(() => {
        let list = plansQuery.data ?? [];
        if (statusFilter === "active") {
            list = list.filter(p => ACTIVE_STATUSES.includes(p.status));
        }
        if (q.trim()) {
            const needle = q.trim().toLowerCase();
            list = list.filter(
                p =>
                    p.title.toLowerCase().includes(needle) ||
                    (p.description?.toLowerCase().includes(needle) ?? false) ||
                    (p.codebaseName?.toLowerCase().includes(needle) ?? false)
            );
        }
        return list;
    }, [plansQuery.data, statusFilter, q]);

    // Row actions
    const updateMutation = useMutation({
        mutationFn: async ({ id, body }: { id: string; body: Record<string, unknown> }) =>
            unwrapEden(await (api.api as any).plans[id].patch(body)),
        onSuccess: () => queryClient.invalidateQueries({ queryKey: ["plans"] }),
        onError: () => toast.error("Failed to update plan"),
    });

    const duplicateMutation = useMutation({
        mutationFn: async (id: string) =>
            unwrapEden(await (api.api as any).plans[id].duplicate.post()) as PlanRow,
        onSuccess: created => {
            queryClient.invalidateQueries({ queryKey: ["plans"] });
            toast.success(`Duplicated as "${created.title}"`);
            navigate({ to: "/plans/$planId", params: { planId: created.id } });
        },
        onError: () => toast.error("Failed to duplicate plan"),
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) =>
            unwrapEden(await (api.api as any).plans[id].delete()),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["plans"] });
            toast.success("Plan deleted");
            setDeleteTarget(null);
        },
        onError: () => toast.error("Failed to delete plan"),
    });

    const [deleteTarget, setDeleteTarget] = useState<{ id: string; title: string } | null>(null);

    const tabs: Array<{ value: StatusFilter; label: string }> = [
        { value: "active", label: "Active" },
        { value: "draft", label: "Draft" },
        { value: "ready", label: "Ready" },
        { value: "completed", label: "Completed" },
        { value: "archived", label: "Archived" },
        { value: "all", label: "All" },
    ];

    return (
        <PageContainer>
            <PageHeader
                title="Plans"
                description="Plans are the home for a unit of work. Each plan holds its description, linked requirements, analyze notes, and tasks."
                actions={
                    <Button render={<Link to="/plans/new" />}>
                        <Plus className="size-4" />
                        New Plan
                    </Button>
                }
            />

            {/* Status tabs */}
            <div className="mb-4 flex flex-wrap items-center gap-1 border-b">
                {tabs.map(t => {
                    const active = statusFilter === t.value;
                    return (
                        <button
                            key={t.value}
                            type="button"
                            onClick={() => setSearch({ status: t.value === "active" ? undefined : t.value })}
                            className={`-mb-px border-b-2 px-3 py-2 text-sm font-medium transition-colors ${
                                active
                                    ? "border-foreground text-foreground"
                                    : "text-muted-foreground hover:text-foreground border-transparent"
                            }`}
                        >
                            {t.label}
                        </button>
                    );
                })}
            </div>

            {/* Toolbar: search + codebase */}
            <div className="mb-4 flex flex-wrap items-center gap-2">
                <div className="relative min-w-0 flex-1">
                    <Search className="text-muted-foreground absolute top-1/2 left-3 size-4 -translate-y-1/2" />
                    <input
                        ref={searchRef}
                        type="text"
                        placeholder="Search plans...  ( / )"
                        value={qDraft}
                        onChange={e => setQDraft(e.target.value)}
                        className="bg-background focus:ring-ring w-full rounded-lg border py-2 pl-9 pr-3 text-sm focus:ring-2 focus:outline-none"
                    />
                    {qDraft && (
                        <button
                            onClick={() => { setQDraft(""); setSearch({ q: undefined }); }}
                            className="text-muted-foreground absolute top-1/2 right-3 -translate-y-1/2"
                        >
                            <X className="size-3.5" />
                        </button>
                    )}
                </div>

                <Select value={codebaseId || "__all__"} onValueChange={v => setSearch({ codebase: !v || v === "__all__" ? undefined : (v as string) })}>
                    <SelectTrigger size="sm" className="w-[180px]">
                        <SelectValue placeholder="All codebases" />
                    </SelectTrigger>
                    <SelectPopup>
                        <SelectItem value="__all__">All codebases</SelectItem>
                        {(codebasesQuery.data ?? []).map(c => (
                            <SelectItem key={c.id} value={c.id}>{c.name}</SelectItem>
                        ))}
                    </SelectPopup>
                </Select>
            </div>

            {/* List */}
            {plansQuery.isLoading ? (
                <PageLoading />
            ) : (
                <div className="grid gap-2">
                    {filteredPlans.map(p => (
                        <PlanRow
                            key={p.id}
                            plan={p}
                            onArchiveToggle={() =>
                                updateMutation.mutate({
                                    id: p.id,
                                    body: { status: p.status === "archived" ? "draft" : "archived" },
                                })
                            }
                            onDuplicate={() => duplicateMutation.mutate(p.id)}
                            onDelete={() => setDeleteTarget({ id: p.id, title: p.title })}
                        />
                    ))}
                    {filteredPlans.length === 0 && (
                        <div className="rounded-md border border-dashed p-8 text-center text-sm text-muted-foreground">
                            {q || codebaseId || statusFilter !== "active"
                                ? "No plans match the current filters."
                                : (
                                    <>
                                        No plans yet.{" "}
                                        <Link to="/plans/new" className="underline">
                                            Create one
                                        </Link>{" "}
                                        to get started.
                                    </>
                                )}
                        </div>
                    )}
                </div>
            )}

            <ConfirmDialog
                open={deleteTarget !== null}
                onOpenChange={open => { if (!open) setDeleteTarget(null); }}
                title="Delete plan"
                description={
                    deleteTarget
                        ? `Delete plan "${deleteTarget.title}"? All linked tasks, analyze items, and requirement links will be removed. This cannot be undone.`
                        : ""
                }
                confirmLabel="Delete"
                confirmVariant="destructive"
                onConfirm={() => { if (deleteTarget) deleteMutation.mutate(deleteTarget.id); }}
                loading={deleteMutation.isPending}
            />
        </PageContainer>
    );
}

interface PlanRowProps {
    plan: PlanRow;
    onArchiveToggle: () => void;
    onDuplicate: () => void;
    onDelete: () => void;
}

function PlanRow({ plan, onArchiveToggle, onDuplicate, onDelete }: PlanRowProps) {
    const progressPct = plan.taskTotal === 0 ? 0 : Math.round((plan.taskDone / plan.taskTotal) * 100);
    const isArchived = plan.status === "archived";
    return (
        <div className={`group flex items-start gap-3 rounded-md border px-4 py-3 transition-colors hover:bg-muted/40 ${isArchived ? "opacity-60" : ""}`}>
            <Link
                to="/plans/$planId"
                params={{ planId: plan.id }}
                className="min-w-0 flex-1"
            >
                <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium">{plan.title}</span>
                    <PlanStatusPill status={plan.status} />
                    {plan.codebaseName && (
                        <span className="text-muted-foreground rounded border px-1.5 py-0.5 text-[10px]">
                            {plan.codebaseName}
                        </span>
                    )}
                </div>
                {plan.description && (
                    <div className="mt-0.5 text-xs text-muted-foreground line-clamp-1">{plan.description}</div>
                )}
                {plan.taskTotal > 0 && (
                    <div className="mt-1.5 flex items-center gap-2 text-xs text-muted-foreground">
                        <div className="bg-muted h-1 max-w-[120px] flex-1 overflow-hidden rounded">
                            <div className="bg-emerald-500 h-full" style={{ width: `${progressPct}%` }} />
                        </div>
                        <span className="font-mono tabular-nums">{plan.taskDone}/{plan.taskTotal}</span>
                    </div>
                )}
                {plan.nextAction && plan.status !== "completed" && plan.status !== "archived" && (
                    <div className="mt-0.5 truncate text-xs">
                        <span className="text-muted-foreground">Next: </span>
                        <span>{plan.nextAction}</span>
                    </div>
                )}
            </Link>
            <div className="flex shrink-0 items-center gap-2 self-start">
                <span className="text-[10px] text-muted-foreground tabular-nums">
                    {new Date(plan.lastActivityAt).toLocaleDateString()}
                </span>
                <DropdownMenu>
                    <DropdownMenuTrigger
                        render={
                            <button
                                aria-label={`Actions for ${plan.title}`}
                                className="text-muted-foreground/0 group-hover:text-muted-foreground hover:text-foreground rounded p-1 transition-colors"
                                onClick={e => e.stopPropagation()}
                            >
                                <MoreHorizontal className="size-4" />
                            </button>
                        }
                    />
                    <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={onDuplicate}>
                            <Copy className="size-3.5" />
                            Duplicate
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={onArchiveToggle}>
                            {isArchived
                                ? (<><ArchiveRestore className="size-3.5" /> Unarchive</>)
                                : (<><Archive className="size-3.5" /> Archive</>)}
                        </DropdownMenuItem>
                        <DropdownMenuSeparator />
                        <DropdownMenuItem onClick={onDelete} className="text-destructive">
                            <Trash2 className="size-3.5" />
                            Delete
                        </DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>
            </div>
        </div>
    );
}
