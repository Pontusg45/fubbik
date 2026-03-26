import { useTheme } from "next-themes";

const TYPE_COLORS_DARK: Record<string, { bg: string; border: string; label: string }> = {
    note: { bg: "#1e293b", border: "#475569", label: "Note" },
    guide: { bg: "#1e1b4b", border: "#6366f1", label: "Guide" },
    reference: { bg: "#042f2e", border: "#14b8a6", label: "Reference" },
    document: { bg: "#172554", border: "#3b82f6", label: "Document" },
    schema: { bg: "#1c1917", border: "#f59e0b", label: "Schema" },
    checklist: { bg: "#1a2e05", border: "#84cc16", label: "Checklist" }
};

const TYPE_COLORS_LIGHT: Record<string, { bg: string; border: string; label: string }> = {
    note: { bg: "#f1f5f9", border: "#94a3b8", label: "Note" },
    guide: { bg: "#eef2ff", border: "#6366f1", label: "Guide" },
    reference: { bg: "#f0fdfa", border: "#14b8a6", label: "Reference" },
    document: { bg: "#eff6ff", border: "#3b82f6", label: "Document" },
    schema: { bg: "#fefce8", border: "#f59e0b", label: "Schema" },
    checklist: { bg: "#f7fee7", border: "#84cc16", label: "Checklist" }
};

const RELATION_COLORS: Record<string, { color: string; label: string }> = {
    related_to: { color: "#6b7280", label: "Related to" },
    part_of: { color: "#8b5cf6", label: "Part of" },
    depends_on: { color: "#f59e0b", label: "Depends on" },
    extends: { color: "#3b82f6", label: "Extends" },
    references: { color: "#06b6d4", label: "References" },
    contradicts: { color: "#ef4444", label: "Contradicts" },
    supports: { color: "#22c55e", label: "Supports" },
    alternative_to: { color: "#ec4899", label: "Alternative to" }
};

export function GraphLegend({ activeTypes, activeRelations, onToggleType, onToggleRelation }: {
    activeTypes: Set<string>;
    activeRelations: Set<string>;
    onToggleType?: (type: string) => void;
    onToggleRelation?: (rel: string) => void;
}) {
    const { resolvedTheme } = useTheme();
    const TYPE_COLORS = resolvedTheme === "light" ? TYPE_COLORS_LIGHT : TYPE_COLORS_DARK;
    const usedTypes = Object.entries(TYPE_COLORS).filter(([key]) => activeTypes.has(key));
    const usedRelations = Object.entries(RELATION_COLORS).filter(([key]) => activeRelations.has(key));

    return (
        <div className="bg-background/80 absolute top-4 left-1/2 z-10 -translate-x-1/2 rounded-lg border px-4 py-2 backdrop-blur-sm">
            <div className="flex items-center gap-4">
                {usedTypes.map(([key, value]) => (
                    <button
                        key={value.label}
                        className="flex items-center gap-1.5 cursor-pointer hover:opacity-80"
                        onClick={() => onToggleType?.(key)}
                    >
                        <div className="size-2.5 rounded-sm border" style={{ background: value.bg, borderColor: value.border }} />
                        <span className="text-muted-foreground text-[10px]">{value.label}</span>
                    </button>
                ))}
                {usedRelations.length > 0 && usedTypes.length > 0 && <div className="bg-border h-3 w-px" />}
                {usedRelations.map(([key, value]) => (
                    <button
                        key={value.label}
                        className="flex items-center gap-1.5 cursor-pointer hover:opacity-80"
                        onClick={() => onToggleRelation?.(key)}
                    >
                        <div className="h-0.5 w-2.5 rounded" style={{ background: value.color }} />
                        <span className="text-muted-foreground text-[10px]">{value.label}</span>
                    </button>
                ))}
            </div>
        </div>
    );
}
