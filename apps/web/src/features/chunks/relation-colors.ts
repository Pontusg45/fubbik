const RELATION_COLORS: Record<string, string> = {
    related_to: "#6b7280",
    part_of: "#8b5cf6",
    depends_on: "#f59e0b",
    extends: "#3b82f6",
    references: "#06b6d4",
    contradicts: "#ef4444",
    supports: "#22c55e",
    alternative_to: "#ec4899"
};

export function relationColor(relation: string): string {
    return RELATION_COLORS[relation] ?? "#6b7280";
}
