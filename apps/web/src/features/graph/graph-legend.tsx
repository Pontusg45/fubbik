import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";

const TYPE_COLORS: Record<string, { bg: string; border: string; label: string }> = {
    note: { bg: "#1e293b", border: "#475569", label: "Note" },
    guide: { bg: "#1e1b4b", border: "#6366f1", label: "Guide" },
    reference: { bg: "#042f2e", border: "#14b8a6", label: "Reference" },
    document: { bg: "#172554", border: "#3b82f6", label: "Document" },
    schema: { bg: "#1c1917", border: "#f59e0b", label: "Schema" },
    checklist: { bg: "#1a2e05", border: "#84cc16", label: "Checklist" }
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

export function GraphLegend({ activeTypes, activeRelations }: {
    activeTypes: Set<string>;
    activeRelations: Set<string>;
}) {
    const [open, setOpen] = useState(false);
    const usedTypes = Object.entries(TYPE_COLORS).filter(([key]) => activeTypes.has(key));
    const usedRelations = Object.entries(RELATION_COLORS).filter(([key]) => activeRelations.has(key));

    return (
        <div className="absolute bottom-4 left-4 z-10 rounded-lg border bg-background/80 backdrop-blur-sm">
            <button
                onClick={() => setOpen(!open)}
                className="flex items-center gap-1.5 px-3 py-2 text-xs font-medium"
            >
                {open ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                Legend
            </button>
            {open && (
                <div className="space-y-3 border-t px-3 py-2">
                    {usedTypes.length > 0 && (
                        <div>
                            <p className="text-muted-foreground mb-1 text-[10px] font-medium uppercase">Node Types</p>
                            <div className="space-y-1">
                                {usedTypes.map(([, value]) => (
                                    <div key={value.label} className="flex items-center gap-2">
                                        <div
                                            className="size-3 rounded-sm border"
                                            style={{ background: value.bg, borderColor: value.border }}
                                        />
                                        <span className="text-xs">{value.label}</span>
                                    </div>
                                ))}
                            </div>
                        </div>
                    )}
                    {usedRelations.length > 0 && (
                        <div>
                            <p className="text-muted-foreground mb-1 text-[10px] font-medium uppercase">Relations</p>
                            <div className="space-y-1">
                                {usedRelations.map(([, value]) => (
                                    <div key={value.label} className="flex items-center gap-2">
                                        <div className="h-0.5 w-3 rounded" style={{ background: value.color }} />
                                        <span className="text-xs">{value.label}</span>
                                    </div>
                                ))}
                            </div>
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
