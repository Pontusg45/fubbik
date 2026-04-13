import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

// ─── Types ───────────────────────────────────────────────────────────────────

type FeedKind = "proposal" | "stale" | "activity";

interface FeedItem {
    id: string;
    kind: FeedKind;
    timestamp: string;
    title: string;
    subtitle?: string;
    // Proposal-specific
    proposalId?: string;
    chunkId?: string;
    // Stale-specific
    flagId?: string;
    // Activity-specific
    action?: string;
    entityType?: string;
    entityId?: string;
}

type TabValue = "all" | FeedKind;

const TABS: { value: TabValue; label: string }[] = [
    { value: "all", label: "All" },
    { value: "proposal", label: "Proposals" },
    { value: "stale", label: "Stale" },
    { value: "activity", label: "Activity" },
];

// ─── Helpers ─────────────────────────────────────────────────────────────────

function formatRelativeTime(date: string | Date): string {
    const now = Date.now();
    const then = new Date(date).getTime();
    const seconds = Math.floor((now - then) / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    if (days < 30) return `${days}d ago`;
    return new Date(date).toLocaleDateString();
}

// ─── Sub-components ───────────────────────────────────────────────────────────

function KindDot({ kind }: { kind: FeedKind }) {
    if (kind === "proposal") {
        return <span className="mt-[5px] size-2 shrink-0 rounded-full bg-amber-500" />;
    }
    if (kind === "stale") {
        return <span className="mt-[5px] size-2 shrink-0 rounded-full bg-amber-500/40" />;
    }
    return <span className="mt-[5px] size-2 shrink-0 rounded-full bg-muted-foreground/40" />;
}

function KindBadge({ kind, action }: { kind: FeedKind; action?: string }) {
    if (kind === "proposal") {
        return (
            <span className="inline-flex items-center rounded border border-amber-500/30 bg-amber-500/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-amber-500">
                Proposal
            </span>
        );
    }
    if (kind === "stale") {
        return (
            <span className="inline-flex items-center rounded border border-amber-500/20 bg-amber-500/5 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-amber-400/70">
                Stale
            </span>
        );
    }
    return (
        <span className="inline-flex items-center rounded border border-border bg-muted px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
            {action ?? "Activity"}
        </span>
    );
}

function ProposalActions({
    proposalId,
    onUpdate,
}: {
    proposalId: string;
    onUpdate: () => void;
}) {
    const approveMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(await (api.api as any).proposals[proposalId].approve.post({})),
        onSuccess: () => {
            toast.success("Proposal approved");
            onUpdate();
        },
        onError: () => toast.error("Failed to approve"),
    });

    const rejectMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(await (api.api as any).proposals[proposalId].reject.post({})),
        onSuccess: () => {
            toast.success("Proposal rejected");
            onUpdate();
        },
        onError: () => toast.error("Failed to reject"),
    });

    const isPending = approveMutation.isPending || rejectMutation.isPending;

    return (
        <div className="mt-1.5 flex gap-2">
            <Button
                size="xs"
                variant="outline"
                className="border-emerald-500/40 text-emerald-600 hover:bg-emerald-500/10"
                onClick={() => approveMutation.mutate()}
                disabled={isPending}
            >
                Approve
            </Button>
            <Button
                size="xs"
                variant="ghost"
                className="text-destructive-foreground hover:text-destructive hover:bg-destructive/10"
                onClick={() => rejectMutation.mutate()}
                disabled={isPending}
            >
                Reject
            </Button>
        </div>
    );
}

// ─── Main component ───────────────────────────────────────────────────────────

export function UnifiedFeed() {
    const queryClient = useQueryClient();
    const [activeTab, setActiveTab] = useState<TabValue>("all");

    const proposalsQuery = useQuery({
        queryKey: ["proposals-pending-feed"],
        queryFn: async () =>
            unwrapEden(await (api.api as any).proposals.get({ query: { status: "pending" } })),
    });

    const staleQuery = useQuery({
        queryKey: ["stale-flags-feed"],
        queryFn: async () =>
            unwrapEden(await api.api.chunks.stale.get({ query: { limit: "10" } })),
    });

    const activityQuery = useQuery({
        queryKey: ["activity-feed"],
        queryFn: async () =>
            unwrapEden(await api.api.activity.get({ query: { limit: "20" } as any })),
    });

    const invalidateProposals = () => {
        queryClient.invalidateQueries({ queryKey: ["proposals-pending-feed"] });
        queryClient.invalidateQueries({ queryKey: ["proposals-count"] });
    };

    // ─── Map sources into FeedItem[] ────────────────────────────────────────

    const proposalItems: FeedItem[] = ((proposalsQuery.data as any) ?? []).map((p: any) => ({
        id: `proposal-${p.id}`,
        kind: "proposal" as FeedKind,
        timestamp: p.createdAt,
        title: p.chunkTitle ?? p.chunkId,
        subtitle: p.reason ?? undefined,
        proposalId: p.id,
        chunkId: p.chunkId,
    }));

    const staleItems: FeedItem[] = ((staleQuery.data as any) ?? []).map((f: any) => ({
        id: `stale-${f.id}`,
        kind: "stale" as FeedKind,
        timestamp: f.createdAt ?? new Date(0).toISOString(),
        title: f.chunkTitle ?? f.chunkId,
        subtitle: f.detail ?? f.reason ?? undefined,
        flagId: f.id,
        chunkId: f.chunkId,
    }));

    const rawActivities = (activityQuery.data as any) ?? [];
    const activityArr = Array.isArray(rawActivities)
        ? rawActivities
        : rawActivities.activities ?? [];

    const activityItems: FeedItem[] = activityArr.map((e: any) => ({
        id: `activity-${e.id}`,
        kind: "activity" as FeedKind,
        timestamp: e.createdAt,
        title: e.entityTitle ?? e.entityId ?? "—",
        subtitle: e.entityType,
        action: e.action,
        entityType: e.entityType,
        entityId: e.entityId,
    }));

    // Merge and sort
    const allItems: FeedItem[] = [...proposalItems, ...staleItems, ...activityItems].sort(
        (a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
    );

    const filtered = activeTab === "all" ? allItems : allItems.filter((i) => i.kind === activeTab);

    const isLoading =
        proposalsQuery.isLoading || staleQuery.isLoading || activityQuery.isLoading;

    return (
        <div>
            {/* Filter tabs */}
            <div className="mb-3 flex gap-1">
                {TABS.map((tab) => (
                    <Button
                        key={tab.value}
                        size="xs"
                        variant={activeTab === tab.value ? "default" : "outline"}
                        onClick={() => setActiveTab(tab.value)}
                    >
                        {tab.label}
                    </Button>
                ))}
            </div>

            {isLoading && (
                <p className="py-6 text-center text-sm text-muted-foreground">Loading…</p>
            )}

            {!isLoading && filtered.length === 0 && (
                <div className="rounded-lg border border-dashed p-8 text-center">
                    <p className="text-sm text-muted-foreground">
                        Nothing happening yet.{" "}
                        <Link to="/chunks/new" className="hover:underline text-foreground">
                            Create a chunk
                        </Link>{" "}
                        or{" "}
                        <Link to="/plans/new" className="hover:underline text-foreground">
                            start a plan
                        </Link>{" "}
                        to get started.
                    </p>
                </div>
            )}

            {!isLoading && filtered.length > 0 && (
                <ul className="space-y-1">
                    {filtered.map((item) => (
                        <li
                            key={item.id}
                            className="flex items-start gap-3 rounded-md px-2 py-2 hover:bg-accent/40"
                        >
                            <KindDot kind={item.kind} />
                            <div className="min-w-0 flex-1">
                                <div className="flex flex-wrap items-center gap-2">
                                    <KindBadge kind={item.kind} action={item.action} />
                                    {item.chunkId && item.kind !== "activity" ? (
                                        <Link
                                            to="/chunks/$chunkId"
                                            params={{ chunkId: item.chunkId }}
                                            className="truncate text-sm font-medium hover:underline"
                                        >
                                            {item.title}
                                        </Link>
                                    ) : (
                                        <span className="truncate text-sm font-medium">
                                            {item.title}
                                        </span>
                                    )}
                                    <span className="ml-auto shrink-0 text-[11px] text-muted-foreground">
                                        {formatRelativeTime(item.timestamp)}
                                    </span>
                                </div>
                                {item.subtitle && (
                                    <p className="mt-0.5 text-xs text-muted-foreground truncate">
                                        {item.subtitle}
                                    </p>
                                )}
                                {item.kind === "proposal" && item.proposalId && (
                                    <ProposalActions
                                        proposalId={item.proposalId}
                                        onUpdate={invalidateProposals}
                                    />
                                )}
                            </div>
                        </li>
                    ))}
                </ul>
            )}
        </div>
    );
}
