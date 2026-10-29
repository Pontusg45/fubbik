export type NavFeature = "plans" | "requirements" | "matrices" | "coordination" | "learning_paths" | "overlays" | "ai";

/** A `null` set (loading, or an older server without /api/capabilities) enables everything. */
export function isFeatureEnabled(enabled: Set<string> | null, feature?: string): boolean {
    return feature === undefined || enabled === null || enabled.has(feature);
}

export function filterNavItems<T extends { feature?: string }>(items: T[], enabled: Set<string> | null): T[] {
    return items.filter(item => isFeatureEnabled(enabled, item.feature));
}
