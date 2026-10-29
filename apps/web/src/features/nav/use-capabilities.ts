import { useQuery } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

/** Enabled feature keys from `GET /api/capabilities`; `null` while loading or on error (callers show everything). */
export function useCapabilities(): Set<string> | null {
    const query = useQuery({
        queryKey: ["capabilities"],
        queryFn: async () => new Set<string>(unwrapEden(await api.api.capabilities.get()).features),
        staleTime: 5 * 60 * 1000,
        retry: false
    });
    return query.isSuccess ? query.data : null;
}
