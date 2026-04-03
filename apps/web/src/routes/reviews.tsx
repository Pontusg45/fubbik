import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { ClipboardList } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { ReviewQueueContent } from "@/features/reviews/review-queue-content";
import { SessionCard } from "@/features/reviews/session-card";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/reviews")({
    component: ReviewsPage,
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

type ActiveTab = "sessions" | "queue";
type StatusFilter = "all" | "in_progress" | "completed" | "reviewed";

const statusOptions: { value: StatusFilter; label: string }[] = [
    { value: "all", label: "All" },
    { value: "in_progress", label: "In Progress" },
    { value: "completed", label: "Completed" },
    { value: "reviewed", label: "Reviewed" }
];

interface Session {
    id: string;
    title: string;
    status: string;
    codebaseId: string | null;
    createdAt: string;
}

function ReviewsPage() {
    const { codebaseId } = useActiveCodebase();
    const [activeTab, setActiveTab] = useState<ActiveTab>("sessions");
    const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
    const [page, setPage] = useState(0);
    const pageSize = 20;

    // Reset page when filter changes
    const handleStatusChange = (status: StatusFilter) => {
        setStatusFilter(status);
        setPage(0);
    };

    // Fetch codebases for name lookup
    const codebasesQuery = useQuery({
        queryKey: ["codebases"],
        queryFn: async () => unwrapEden(await api.api.codebases.get()),
        staleTime: 60_000
    });
    const codebaseMap = new Map(
        ((codebasesQuery.data as Array<{ id: string; name: string }>) ?? []).map(c => [c.id, c.name])
    );

    const sessionsQuery = useQuery({
        queryKey: ["sessions", codebaseId, statusFilter, page],
        queryFn: async () => {
            const query: { status?: string; codebaseId?: string; limit?: string; offset?: string } = {
                limit: String(pageSize),
                offset: String(page * pageSize)
            };
            if (statusFilter !== "all") query.status = statusFilter;
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await api.api.sessions.get({ query }));
        },
        enabled: activeTab === "sessions"
    });

    // Fetch all sessions (unfiltered) for counts
    const allSessionsQuery = useQuery({
        queryKey: ["sessions-counts", codebaseId],
        queryFn: async () => {
            const query: { codebaseId?: string; limit?: string } = { limit: "500" };
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await api.api.sessions.get({ query }));
        },
        enabled: activeTab === "sessions",
        staleTime: 30_000
    });

    const allSessions = (allSessionsQuery.data as { sessions: Session[]; total: number } | undefined)?.sessions ?? [];
    const statusCounts: Record<string, number> = {};
    for (const s of allSessions) {
        statusCounts[s.status] = (statusCounts[s.status] ?? 0) + 1;
    }

    const data = sessionsQuery.data as { sessions: Session[]; total: number } | undefined;
    const sessions = data?.sessions ?? [];
    const total = data?.total ?? 0;

    return (
        <div className="container mx-auto max-w-4xl px-4 py-8">
            {/* Header */}
            <div className="mb-6 flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <ClipboardList className="size-5" />
                    <h1 className="text-2xl font-bold tracking-tight">Reviews</h1>
                </div>
            </div>

            {/* Tab switcher */}
            <div className="flex gap-1 mb-4">
                {(["sessions", "queue"] as const).map(tab => (
                    <Button
                        key={tab}
                        variant={activeTab === tab ? "default" : "outline"}
                        size="sm"
                        onClick={() => setActiveTab(tab)}
                    >
                        {tab === "sessions" ? "Sessions" : "Review Queue"}
                    </Button>
                ))}
            </div>

            {activeTab === "sessions" && (
                <>
                    {/* Status filter */}
                    <div className="mb-6 flex gap-2">
                        {statusOptions.map(opt => {
                            const count = opt.value === "all"
                                ? allSessions.length
                                : statusCounts[opt.value] ?? 0;
                            return (
                                <Button
                                    key={opt.value}
                                    variant={statusFilter === opt.value ? "default" : "outline"}
                                    size="sm"
                                    onClick={() => handleStatusChange(opt.value)}
                                >
                                    {opt.label}{count > 0 ? ` (${count})` : ""}
                                </Button>
                            );
                        })}
                    </div>

                    {/* Content */}
                    {sessionsQuery.isLoading ? (
                        <Card>
                            <CardPanel className="p-6">
                                <p className="text-muted-foreground text-sm">Loading...</p>
                            </CardPanel>
                        </Card>
                    ) : sessions.length === 0 ? (
                        <Card>
                            <CardPanel className="p-6">
                                <div className="flex flex-col items-center gap-3 py-12">
                                    <ClipboardList className="text-muted-foreground/20 size-10" />
                                    <div className="text-center">
                                        <p className="text-muted-foreground font-medium">No implementation sessions yet</p>
                                        <p className="text-muted-foreground/70 mt-1 text-sm">
                                            Sessions are created when you start an implementation review via the CLI or API.
                                        </p>
                                    </div>
                                </div>
                            </CardPanel>
                        </Card>
                    ) : (
                        <>
                            <div className="space-y-3">
                                {sessions.map(session => (
                                    <SessionCard
                                        key={session.id}
                                        id={session.id}
                                        title={session.title}
                                        status={session.status}
                                        codebaseName={session.codebaseId ? codebaseMap.get(session.codebaseId) : undefined}
                                        createdAt={session.createdAt}
                                    />
                                ))}
                            </div>

                            {/* Pagination */}
                            {total > pageSize && (
                                <div className="text-muted-foreground mt-4 flex items-center justify-between text-sm">
                                    <span>
                                        Showing {page * pageSize + 1}-{Math.min((page + 1) * pageSize, total)} of {total}
                                    </span>
                                    <div className="flex gap-2">
                                        <Button variant="outline" size="sm" disabled={page === 0} onClick={() => setPage(p => p - 1)}>
                                            Previous
                                        </Button>
                                        <Button
                                            variant="outline"
                                            size="sm"
                                            disabled={(page + 1) * pageSize >= total}
                                            onClick={() => setPage(p => p + 1)}
                                        >
                                            Next
                                        </Button>
                                    </div>
                                </div>
                            )}
                        </>
                    )}
                </>
            )}

            {activeTab === "queue" && <ReviewQueueContent />}
        </div>
    );
}
