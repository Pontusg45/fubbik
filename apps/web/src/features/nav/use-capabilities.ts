import { useQuery } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export type Feature = "plans" | "requirements" | "matrices" | "coordination" | "learning_paths" | "overlays" | "ai";

export const FEATURE_BY_ROUTE: Record<string, Feature> = {
    "/plans": "plans",
    "/requirements": "requirements",
    "/coverage": "requirements",
    "/matrices": "matrices",
    "/learn": "learning_paths",
    "/features": "overlays"
};

function featureFor(to: string): Feature | undefined {
    return FEATURE_BY_ROUTE[to] ?? FEATURE_BY_ROUTE[`/${to.split("/")[1] ?? ""}`];
}

/** A `null` set (loading, or an older server without /api/capabilities) keeps every item. */
export function filterNavItems<T extends { to: string }>(items: T[], enabled: Set<string> | null): T[] {
    if (enabled === null) return items;
    return items.filter(item => {
        const feature = featureFor(item.to);
        return feature === undefined || enabled.has(feature);
    });
}

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
