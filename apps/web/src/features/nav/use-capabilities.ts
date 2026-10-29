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
export function filterNavItems<T extends { to: string }>(items: readonly T[], enabled: Set<string> | null): T[] {
    if (enabled === null) return [...items];
    return items.filter(item => {
        const feature = featureFor(item.to);
        return feature === undefined || enabled.has(feature);
    });
}

/**
 * Fetches `GET /api/capabilities`. Never throws: a 404 (an older server
 * without the endpoint) or a network error resolves to `null`, so the global
 * `QueryCache.onError` toast never fires for this query.
 */
export async function fetchCapabilities(
    get: () => Promise<{ data: unknown; error: unknown }> = () => api.api.capabilities.get()
): Promise<Set<string> | null> {
    try {
        const { features } = unwrapEden(await get()) as { features: string[] };
        return Array.isArray(features) ? new Set<string>(features) : null;
    } catch {
        return null;
    }
}

/** Enabled feature keys from `GET /api/capabilities`; `null` while loading or on error (callers show everything). */
export function useCapabilities(): Set<string> | null {
    const query = useQuery({
        queryKey: ["capabilities"],
        queryFn: () => fetchCapabilities(),
        staleTime: 5 * 60 * 1000,
        retry: false,
        refetchOnWindowFocus: false
    });
    return query.data ?? null;
}

/**
 * The single rule for gating UI on a feature switch: `true`/`false` once
 * capabilities are known, `null` while they are unknown (loading, or a server
 * without `/api/capabilities`). Callers skip work only on `false`, so an
 * unknown state keeps today's behaviour.
 */
export function isFeatureEnabled(capabilities: Set<string> | null, feature: Feature): boolean | null {
    return capabilities === null ? null : capabilities.has(feature);
}

export function useFeatureEnabled(feature: Feature): boolean | null {
    return isFeatureEnabled(useCapabilities(), feature);
}
