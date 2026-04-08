import { X } from "lucide-react";
import { FILTER_COLORS } from "./query-types";
import type { QueryClause } from "./query-types";

interface FilterPillsProps {
    clauses: QueryClause[];
    join: "and" | "or";
    onRemove: (index: number) => void;
    onSetJoin: (join: "and" | "or") => void;
}

function operatorLabel(operator: string): string {
    switch (operator) {
        case "eq": return "is";
        case "neq": return "is not";
        case "gt": return ">";
        case "lt": return "<";
        case "gte": return ">=";
        case "lte": return "<=";
        case "contains": return "contains";
        default: return operator;
    }
}

export function FilterPills({ clauses, join, onRemove, onSetJoin }: FilterPillsProps) {
    if (clauses.length === 0) return null;

    return (
        <div className="flex flex-wrap items-center gap-1.5">
            {clauses.map((clause, index) => {
                const colorClass = FILTER_COLORS[clause.field] ?? "bg-slate-500/15 border-slate-500/30 text-slate-400";
                return (
                    <div key={index} className="flex items-center gap-1.5">
                        {index > 0 && (
                            <button
                                onClick={() => onSetJoin(join === "and" ? "or" : "and")}
                                className="text-muted-foreground hover:text-foreground rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider transition-colors hover:bg-muted"
                                title="Click to toggle AND/OR"
                            >
                                {join}
                            </button>
                        )}
                        <div
                            className={`flex items-center gap-1 rounded border px-2 py-0.5 text-xs ${colorClass}`}
                        >
                            {clause.negate && (
                                <span className="font-semibold opacity-70">NOT</span>
                            )}
                            <span className="font-semibold">{clause.field}</span>
                            <span className="opacity-70">{operatorLabel(clause.operator)}</span>
                            <span>{clause.value}</span>
                            <button
                                onClick={() => onRemove(index)}
                                className="ml-0.5 rounded opacity-60 transition-opacity hover:opacity-100"
                                aria-label="Remove filter"
                            >
                                <X className="size-3" />
                            </button>
                        </div>
                    </div>
                );
            })}
        </div>
    );
}
