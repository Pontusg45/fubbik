import { queryClient } from "@/router";

/**
 * Relation colors — canonical source is the connection_relation catalog (DB-driven),
 * read from the react-query cache for sync access. Falls back to this module-local
 * default palette for first paint / offline / unknown relations.
 *
 * Prefer `useRelationColor` from `@/features/vocabularies/use-vocabularies` in new
 * React components. This sync function exists so older callers don't have to change shape.
 */

const FALLBACK_COLORS: Record<string, string> = {
    related_to: "#94a3b8",
    part_of: "#3b82f6",
    depends_on: "#f59e0b",
    extends: "#6366f1",
    references: "#14b8a6",
    contradicts: "#ef4444",
    supports: "#22c55e",
    alternative_to: "#a855f7"
};

export function relationColor(relation: string): string {
    const cached = queryClient.getQueryData<Array<{ id: string; color: string }>>(["connection-relations", null]);
    if (cached) {
        const hit = cached.find(r => r.id === relation);
        if (hit) return hit.color;
    }
    return FALLBACK_COLORS[relation] ?? "#6b7280";
}
