import { useQuery } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function usePendingProposalCount(): number {
    const { data } = useQuery({
        queryKey: ["proposals-count"],
        queryFn: async () => unwrapEden(await (api.api as any).proposals.count.get()),
        refetchInterval: 60_000,
        staleTime: 30_000,
    });
    return (data as any)?.pending ?? 0;
}
