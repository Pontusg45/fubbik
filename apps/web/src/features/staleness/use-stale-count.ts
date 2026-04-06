import { useQuery } from "@tanstack/react-query";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function useStaleCount() {
    const query = useQuery({
        queryKey: ["stale-count"],
        queryFn: async () => unwrapEden(await api.api.chunks.stale.count.get({ query: {} })),
        refetchInterval: 5 * 60 * 1000,
    });
    return query.data ?? 0;
}
