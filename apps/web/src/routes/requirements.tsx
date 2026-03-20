import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { BarChart3, ClipboardCheck, Plus } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { BulkActions } from "@/features/requirements/bulk-actions";
import { SidebarFilters } from "@/features/requirements/sidebar-filters";
import { SortableRequirementList } from "@/features/requirements/sortable-requirement-list";
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

interface UseCase {
    id: string;
    name: string;
    description: string | null;
    requirementCount: number;
    parentId: string | null;
}

function RequirementsPage() {
    const { codebaseId } = useActiveCodebase();

    // Filter state
    const [search, setSearch] = useState("");
    const [statusFilters, setStatusFilters] = useState<string[]>([]);
    const [priorityFilters, setPriorityFilters] = useState<string[]>([]);
    const [originFilter, setOriginFilter] = useState("");
    const [activeUseCaseId, setActiveUseCaseId] = useState<string | null>(null);
    const [selectedIds, setSelectedIds] = useState<string[]>([]);
    const [page, setPage] = useState(0);
    const pageSize = 20;

    // Reset page when filters change
    const handleSearchChange = (v: string) => { setSearch(v); setPage(0); };
    const handleStatusFiltersChange = (v: string[]) => { setStatusFilters(v); setPage(0); };
    const handlePriorityFiltersChange = (v: string[]) => { setPriorityFilters(v); setPage(0); };
    const handleOriginFilterChange = (v: string) => { setOriginFilter(v); setPage(0); };
    const handleUseCaseClick = (id: string | null) => { setActiveUseCaseId(id); setPage(0); };

    // Queries
    const statsQuery = useQuery({
        queryKey: ["requirements-stats", codebaseId],
        queryFn: async () => {
            const query: { codebaseId?: string } = {};
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await api.api.requirements.stats.get({ query }));
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

    const useCases = (useCasesQuery.data ?? []) as UseCase[];

    const activeUseCaseIds = useMemo(() => {
        if (!activeUseCaseId) return null;
        const ids = new Set([activeUseCaseId]);
        // Add children if this is a parent
        for (const uc of useCases) {
            if (uc.parentId === activeUseCaseId) {
                ids.add(uc.id);
            }
        }
        return ids;
    }, [activeUseCaseId, useCases]);

    const listQuery = useQuery({
        queryKey: [
            "requirements",
            codebaseId,
            search,
            statusFilters.length === 1 ? statusFilters[0] : "",
            priorityFilters.length === 1 ? priorityFilters[0] : "",
            originFilter,
            activeUseCaseId,
            page
        ],
        queryFn: async () => {
            const query: {
                codebaseId?: string;
                search?: string;
                status?: string;
                priority?: string;
                origin?: "human" | "ai";
                useCaseId?: string;
                limit?: string;
                offset?: string;
            } = {
                limit: String(pageSize),
                offset: String(page * pageSize)
            };
            if (codebaseId) query.codebaseId = codebaseId;
            if (search) query.search = search;
            if (statusFilters.length === 1) query.status = statusFilters[0];
            if (priorityFilters.length === 1) query.priority = priorityFilters[0];
            if (originFilter) query.origin = originFilter as "human" | "ai";
            // If parent use case with children, fetch all and filter client-side
            if (activeUseCaseIds && activeUseCaseIds.size === 1) {
                query.useCaseId = activeUseCaseId!;
            }
            return unwrapEden(await api.api.requirements.get({ query }));
        }
    });

    const stats = statsQuery.data as { total: number; passing: number; failing: number; untested: number } | undefined;
    const data = listQuery.data as { requirements: Array<Record<string, unknown>>; total: number } | undefined;
    const useCaseMap = new Map(useCases.map(uc => [uc.id, uc]));

    // Client-side post-filtering for multi-select status/priority and parent use cases
    const filteredRequirements = useMemo(() => {
        if (!data?.requirements) return [];
        let reqs = data.requirements;
        if (statusFilters.length > 1) {
            reqs = reqs.filter(r => statusFilters.includes(r.status as string));
        }
        if (priorityFilters.length > 1) {
            reqs = reqs.filter(r => priorityFilters.includes(r.priority as string));
        }
        if (activeUseCaseIds && activeUseCaseIds.size > 0) {
            reqs = reqs.filter(r => activeUseCaseIds.has(r.useCaseId as string));
        }
        return reqs;
    }, [data, statusFilters, priorityFilters, activeUseCaseIds]);

    function toggleSelection(id: string, selected: boolean) {
        setSelectedIds(prev =>
            selected ? [...prev, id] : prev.filter(i => i !== id)
        );
    }

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            {/* Header */}
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
                    <Button variant="outline" size="sm" render={<Link to="/coverage" />}>
                        <BarChart3 className="mr-1 size-4" />
                        Coverage
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

            {/* Main layout: sidebar + cards */}
            <div className="flex gap-6">
                {/* Sidebar */}
                <div className="w-64 shrink-0">
                    <SidebarFilters
                        search={search}
                        onSearchChange={handleSearchChange}
                        statusFilters={statusFilters}
                        onStatusFiltersChange={handleStatusFiltersChange}
                        priorityFilters={priorityFilters}
                        onPriorityFiltersChange={handlePriorityFiltersChange}
                        originFilter={originFilter}
                        onOriginFilterChange={handleOriginFilterChange}
                        useCases={useCases}
                        activeUseCaseId={activeUseCaseId}
                        onUseCaseClick={handleUseCaseClick}
                        statusCounts={stats ? { passing: stats.passing, failing: stats.failing, untested: stats.untested } : undefined}
                    />
                </div>

                {/* Main content */}
                <div className="flex-1 space-y-3">
                    {listQuery.isLoading ? (
                        <Card>
                            <CardPanel className="p-6">
                                <SkeletonList count={5} />
                            </CardPanel>
                        </Card>
                    ) : filteredRequirements.length === 0 ? (
                        <Card>
                            <CardPanel className="p-6">
                                <div className="flex flex-col items-center gap-3 py-12">
                                    <ClipboardCheck className="text-muted-foreground/20 size-10" />
                                    <div className="text-center">
                                        <p className="text-muted-foreground font-medium">No requirements found</p>
                                        <p className="text-muted-foreground/70 mt-1 text-sm">
                                            {statusFilters.length > 0 || priorityFilters.length > 0 || search || originFilter
                                                ? "Try adjusting your filters."
                                                : "Define requirements to track what your knowledge base should cover."}
                                        </p>
                                    </div>
                                    {!search && statusFilters.length === 0 && (
                                        <Button size="sm" render={<Link to="/requirements/new" />}>
                                            <Plus className="size-3.5" />
                                            Create your first requirement
                                        </Button>
                                    )}
                                </div>
                            </CardPanel>
                        </Card>
                    ) : (
                        <>
                            <SortableRequirementList
                                requirements={filteredRequirements}
                                selectedIds={selectedIds}
                                onToggleSelection={toggleSelection}
                                useCaseMap={useCaseMap}
                            />

                            {/* Pagination */}
                            {data && data.total > pageSize && (
                                <div className="mt-4 flex items-center justify-between text-sm text-muted-foreground">
                                    <span>
                                        Showing {page * pageSize + 1}-{Math.min((page + 1) * pageSize, data.total)} of {data.total}
                                    </span>
                                    <div className="flex gap-2">
                                        <Button
                                            variant="outline"
                                            size="sm"
                                            disabled={page === 0}
                                            onClick={() => setPage(p => p - 1)}
                                        >
                                            Previous
                                        </Button>
                                        <Button
                                            variant="outline"
                                            size="sm"
                                            disabled={(page + 1) * pageSize >= data.total}
                                            onClick={() => setPage(p => p + 1)}
                                        >
                                            Next
                                        </Button>
                                    </div>
                                </div>
                            )}
                        </>
                    )}
                </div>
            </div>

            {/* Bulk actions */}
            <BulkActions
                selectedIds={selectedIds}
                onClearSelection={() => setSelectedIds([])}
                useCases={useCases}
            />
        </div>
    );
}
