import { useQuery } from "@tanstack/react-query";

import { legacyApi } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function usePendingProposalCount(): number {
    const { data } = useQuery({
        queryKey: ["proposals-count"],
        // proposals is a Node-only domain (no Rust route) — must stay on legacyApi.
        queryFn: async () => unwrapEden(await legacyApi.api.proposals.count.get()),
        refetchInterval: 60_000,
        staleTime: 30_000
    });
    return (data as any)?.pending ?? 0;
}
