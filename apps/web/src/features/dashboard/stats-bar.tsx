import { useQuery } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

function Dot() {
    return <span className="text-muted-foreground/40 select-none">·</span>;
}

export function StatsBar() {
    const statsQuery = useQuery({
        queryKey: ["stats"],
        queryFn: async () => unwrapEden(await api.api.stats.get({ query: {} as any })),
    });

    const proposalsQuery = useQuery({
        queryKey: ["proposals-count"],
        queryFn: async () => unwrapEden(await (api.api as any).proposals.count.get()),
        staleTime: 30_000,
        refetchInterval: 60_000,
    });

    const staleQuery = useQuery({
        queryKey: ["stale-count"],
        queryFn: async () => unwrapEden(await api.api.chunks.stale.count.get({ query: {} })),
        refetchInterval: 5 * 60 * 1000,
    });

    if (statsQuery.isLoading) return null;

    const stats = statsQuery.data as any;
    const pendingProposals = (proposalsQuery.data as any)?.pending ?? 0;
    const staleCount = (staleQuery.data as any) ?? 0;

    return (
        <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
            <span>{stats?.chunks ?? 0} chunks</span>
            <Dot />
            <span>{stats?.connections ?? 0} connections</span>
            <Dot />
            <span>{stats?.requirements ?? 0} requirements</span>
            <Dot />
            <span className={pendingProposals > 0 ? "text-amber-500 font-medium" : ""}>
                {pendingProposals} pending proposals
            </span>
            <Dot />
            <span className={staleCount > 0 ? "text-amber-500 font-medium" : ""}>
                {staleCount} stale
            </span>
        </div>
    );
}
