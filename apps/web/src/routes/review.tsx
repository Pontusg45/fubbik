import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { PageContainer, PageHeader, PageLoading } from "@/components/ui/page";
import { ProposalCard, type Proposal } from "@/features/proposals/proposal-card";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/review")({ component: ReviewPage });

function ReviewPage() {
    const [statusFilter, setStatusFilter] = useState<string>("pending");

    const proposalsQuery = useQuery({
        queryKey: ["proposals", statusFilter],
        queryFn: async () => {
            const params = new URLSearchParams();
            if (statusFilter !== "all") params.set("status", statusFilter);
            return unwrapEden(await (api.api as any).proposals.get({ query: Object.fromEntries(params) }));
        },
    });

    const countQuery = useQuery({
        queryKey: ["proposals-count"],
        queryFn: async () => unwrapEden(await (api.api as any).proposals.count.get()),
    });

    const bulkApproveMutation = useMutation({
        mutationFn: async () => {
            const pending = ((proposalsQuery.data ?? []) as Proposal[]).filter(p => p.status === "pending");
            const actions = pending.map(p => ({ proposalId: p.id, action: "approve" as const }));
            return unwrapEden(await (api.api as any).proposals.bulk.post({ actions }));
        },
        onSuccess: () => {
            void proposalsQuery.refetch();
            void countQuery.refetch();
        },
    });

    const bulkRejectMutation = useMutation({
        mutationFn: async () => {
            const pending = ((proposalsQuery.data ?? []) as Proposal[]).filter(p => p.status === "pending");
            const actions = pending.map(p => ({ proposalId: p.id, action: "reject" as const }));
            return unwrapEden(await (api.api as any).proposals.bulk.post({ actions }));
        },
        onSuccess: () => {
            void proposalsQuery.refetch();
            void countQuery.refetch();
        },
    });

    const proposals = (proposalsQuery.data ?? []) as Proposal[];
    const pendingCount = (countQuery.data as any)?.pending ?? 0;
    const hasPending = proposals.some(p => p.status === "pending");

    const refetch = () => {
        void proposalsQuery.refetch();
        void countQuery.refetch();
    };

    const STATUS_OPTIONS = ["pending", "approved", "rejected", "all"] as const;

    return (
        <PageContainer>
            <PageHeader
                title="Review Queue"
                description={`${pendingCount} proposal${pendingCount === 1 ? "" : "s"} waiting for review`}
            />
            <div className="mb-4 flex items-center justify-between">
                <div className="flex gap-1">
                    {STATUS_OPTIONS.map(s => (
                        <Button
                            key={s}
                            size="sm"
                            variant={statusFilter === s ? "default" : "ghost"}
                            onClick={() => setStatusFilter(s)}
                        >
                            {s.charAt(0).toUpperCase() + s.slice(1)}
                        </Button>
                    ))}
                </div>
                {hasPending && (
                    <div className="flex gap-2">
                        <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => {
                                if (confirm("Approve all visible pending proposals?")) bulkApproveMutation.mutate();
                            }}
                        >
                            Approve all
                        </Button>
                        <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => {
                                if (confirm("Reject all visible pending proposals?")) bulkRejectMutation.mutate();
                            }}
                        >
                            Reject all
                        </Button>
                    </div>
                )}
            </div>
            {proposalsQuery.isLoading ? (
                <PageLoading />
            ) : proposals.length === 0 ? (
                <div className="rounded-md border border-dashed p-8 text-center text-sm text-muted-foreground">
                    No proposals waiting for review
                </div>
            ) : (
                <div className="space-y-2">
                    {proposals.map(p => (
                        <ProposalCard key={p.id} proposal={p} onUpdate={refetch} />
                    ))}
                </div>
            )}
        </PageContainer>
    );
}
