import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ClipboardCheck, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
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

function RequirementsPage() {
    const queryClient = useQueryClient();
    const { codebaseId } = useActiveCodebase();
    const [statusFilter, setStatusFilter] = useState("");
    const [priorityFilter, setPriorityFilter] = useState("");

    const statsQuery = useQuery({
        queryKey: ["requirements-stats", codebaseId],
        queryFn: async () => {
            try {
                const query: { codebaseId?: string } = {};
                if (codebaseId) query.codebaseId = codebaseId;
                return unwrapEden(await api.api.requirements.stats.get({ query }));
            } catch {
                return { total: 0, passing: 0, failing: 0, untested: 0 };
            }
        }
    });

    const listQuery = useQuery({
        queryKey: ["requirements", codebaseId, statusFilter, priorityFilter],
        queryFn: async () => {
            try {
                const query: { codebaseId?: string; status?: string; priority?: string; limit?: string } = {
                    limit: "50"
                };
                if (codebaseId) query.codebaseId = codebaseId;
                if (statusFilter) query.status = statusFilter;
                if (priorityFilter) query.priority = priorityFilter;
                return unwrapEden(await api.api.requirements.get({ query }));
            } catch {
                return { requirements: [], total: 0 };
            }
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => {
            return unwrapEden(await api.api.requirements({ id }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            queryClient.invalidateQueries({ queryKey: ["requirements-stats"] });
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

    const stats = statsQuery.data as { total: number; passing: number; failing: number; untested: number } | undefined;
    const data = listQuery.data as { requirements: Array<Record<string, unknown>>; total: number } | undefined;
    const requirements = data?.requirements ?? [];

    function handleDelete(id: string, title: string) {
        if (!confirm(`Delete requirement "${title}"?`)) return;
        deleteMutation.mutate(id);
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
                <Button size="sm" render={<Link to={"/requirements/new" as string} />}>
                    <Plus className="mr-1 size-4" />
                    New Requirement
                </Button>
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

            {/* Filters */}
            <div className="mb-4 flex gap-3">
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
            </div>

            {/* List */}
            <Card>
                <CardPanel className="p-6">
                    {listQuery.isLoading ? (
                        <p className="text-muted-foreground text-sm">Loading...</p>
                    ) : requirements.length === 0 ? (
                        <p className="text-muted-foreground text-sm">No requirements yet.</p>
                    ) : (
                        <div className="divide-y">
                            {requirements.map(req => {
                                const id = req.id as string;
                                const title = req.title as string;
                                const status = (req.status as string) ?? "untested";
                                const priority = req.priority as string | null;
                                const steps = req.steps as Array<unknown>;
                                const createdAt = req.createdAt as string;

                                return (
                                    <div key={id} className="flex items-center justify-between gap-4 py-3 first:pt-0 last:pb-0">
                                        <div className="min-w-0 flex-1">
                                            <div className="flex items-center gap-2">
                                                <Link
                                                    to={"/requirements/$requirementId" as string}
                                                    params={{ requirementId: id } as Record<string, string>}
                                                    className="truncate font-medium hover:underline"
                                                >
                                                    {title}
                                                </Link>
                                                <Badge variant="outline" size="sm" className={statusColor(status)}>
                                                    {status}
                                                </Badge>
                                                {priority && (
                                                    <Badge variant="secondary" size="sm">
                                                        {priorityLabel(priority)}
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
                            })}
                        </div>
                    )}
                </CardPanel>
            </Card>
        </div>
    );
}
