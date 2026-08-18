import { useQuery } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

function Dot() {
    return <span className="text-muted-foreground/40 select-none">·</span>;
}

export function StatsBar() {
    const statsQuery = useQuery({
        queryKey: ["stats"],
        queryFn: async () => unwrapEden(await api.api.stats.get({ query: {} as any }))
    });

    const proposalsQuery = useQuery({
        queryKey: ["proposals-count"],
        queryFn: async () => unwrapEden(await api.api.proposals.count.get()),
        staleTime: 30_000,
        refetchInterval: 60_000
    });

    const staleQuery = useQuery({
        queryKey: ["stale-count"],
        queryFn: async () => unwrapEden(await api.api.chunks.stale.count.get({ query: {} })),
        refetchInterval: 5 * 60 * 1000
    });

    if (statsQuery.isLoading) {
        return (
            <div className="flex animate-pulse items-center gap-2 text-sm">
                <div className="bg-muted h-4 w-20 rounded" />
                <Dot />
                <div className="bg-muted h-4 w-24 rounded" />
                <Dot />
                <div className="bg-muted h-4 w-24 rounded" />
                <Dot />
                <div className="bg-muted h-4 w-28 rounded" />
                <Dot />
                <div className="bg-muted h-4 w-16 rounded" />
            </div>
        );
    }

    const stats = statsQuery.data as any;
    const pendingProposals = (proposalsQuery.data as any)?.pending ?? 0;
    const staleCount = (staleQuery.data as any) ?? 0;

    return (
        <div className="text-muted-foreground flex flex-wrap items-center gap-2 text-sm">
            <span>{stats?.chunks ?? 0} chunks</span>
            <Dot />
            <span>{stats?.connections ?? 0} connections</span>
            <Dot />
            <span>{stats?.requirements ?? 0} requirements</span>
            <Dot />
            <span className={pendingProposals > 0 ? "font-medium text-amber-500" : ""}>{pendingProposals} pending proposals</span>
            <Dot />
            <span className={staleCount > 0 ? "font-medium text-amber-500" : ""}>{staleCount} stale</span>
        </div>
    );
}
