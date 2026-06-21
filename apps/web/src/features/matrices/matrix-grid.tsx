import { Check, ChevronDown, ChevronRight, Code2 } from "lucide-react";
import { useState } from "react";

export interface ViewCell {
    id: string;
    status: "specified" | "unspecified" | "violated" | "verified";
    requirementCount: number;
    codeCount?: number;
    passingTestCount?: number;
    failingTestCount?: number;
}

export interface Dimension {
    id: string;
    name: string;
    order: number;
}

export interface Rule {
    id: string;
    title: string;
    description: string | null;
    category: string | null;
    order: number;
}

interface MatrixGridProps {
    dimensions: Dimension[];
    rules: Rule[];
    /** Key format: "ruleId:dimensionId" */
    cells: Record<string, ViewCell | null>;
    onCellClick: (cell: ViewCell, ruleId: string, dimensionId: string) => void;
    onToggleCell: (ruleId: string, dimensionId: string) => void;
}

const STATUS_COLORS: Record<string, string> = {
    specified: "bg-emerald-500/20 hover:bg-emerald-500/30 border-emerald-500/40 text-emerald-700 dark:text-emerald-300",
    verified: "bg-green-600/30 hover:bg-green-600/40 border-green-600/60 text-green-800 dark:text-green-200",
    unspecified: "bg-amber-500/20 hover:bg-amber-500/30 border-amber-500/40 text-amber-700 dark:text-amber-300",
    violated: "bg-red-500/20 hover:bg-red-500/30 border-red-500/40 text-red-700 dark:text-red-300"
};

const EMPTY_CELL = "bg-muted/30 hover:bg-muted/60 border-muted-foreground/10 text-muted-foreground";

export function MatrixGrid({ dimensions, rules, cells, onCellClick, onToggleCell }: MatrixGridProps) {
    // Group rules by category
    const categorized = groupByCategory(rules);
    const [collapsedCategories, setCollapsedCategories] = useState<Set<string>>(new Set());

    function toggleCategory(category: string) {
        setCollapsedCategories(prev => {
            const next = new Set(prev);
            if (next.has(category)) {
                next.delete(category);
            } else {
                next.add(category);
            }
            return next;
        });
    }

    if (dimensions.length === 0 || rules.length === 0) {
        return (
            <div className="text-muted-foreground rounded-lg border border-dashed py-12 text-center text-sm">
                {dimensions.length === 0 && rules.length === 0
                    ? "Add dimensions and rules to build the matrix."
                    : dimensions.length === 0
                      ? "Add at least one dimension to see the grid."
                      : "Add at least one rule to see the grid."}
            </div>
        );
    }

    return (
        <div className="overflow-x-auto rounded-lg border">
            <table className="w-full border-collapse text-sm">
                <thead>
                    <tr>
                        <th className="bg-muted/50 border-r border-b px-3 py-2 text-left font-medium">Rule / Dimension</th>
                        {dimensions.map(dim => (
                            <th
                                key={dim.id}
                                className="bg-muted/50 min-w-[80px] border-b px-3 py-2 text-center font-medium"
                                title={dim.name}
                            >
                                <span className="line-clamp-2">{dim.name}</span>
                            </th>
                        ))}
                    </tr>
                </thead>
                <tbody>
                    {categorized.map(group => {
                        const isCollapsed = collapsedCategories.has(group.category);
                        return (
                            <CategoryGroup
                                key={group.category}
                                category={group.category}
                                rules={group.rules}
                                dimensions={dimensions}
                                cells={cells}
                                isCollapsed={isCollapsed}
                                onToggleCategory={() => toggleCategory(group.category)}
                                onCellClick={onCellClick}
                                onToggleCell={onToggleCell}
                                showCategoryHeader={categorized.length > 1}
                            />
                        );
                    })}
                </tbody>
            </table>
        </div>
    );
}

interface CategoryGroupProps {
    category: string;
    rules: Rule[];
    dimensions: Dimension[];
    cells: Record<string, ViewCell | null>;
    isCollapsed: boolean;
    onToggleCategory: () => void;
    onCellClick: (cell: ViewCell, ruleId: string, dimensionId: string) => void;
    onToggleCell: (ruleId: string, dimensionId: string) => void;
    showCategoryHeader: boolean;
}

function CategoryGroup({
    category,
    rules,
    dimensions,
    cells,
    isCollapsed,
    onToggleCategory,
    onCellClick,
    onToggleCell,
    showCategoryHeader
}: CategoryGroupProps) {
    return (
        <>
            {showCategoryHeader && (
                <tr>
                    <td colSpan={dimensions.length + 1} className="bg-muted/30 border-b px-3 py-1.5">
                        <button
                            type="button"
                            onClick={onToggleCategory}
                            className="text-muted-foreground hover:text-foreground flex items-center gap-1.5 text-xs font-semibold tracking-wide uppercase transition-colors"
                        >
                            {isCollapsed ? <ChevronRight className="size-3.5" /> : <ChevronDown className="size-3.5" />}
                            {category}
                            <span className="font-normal normal-case">({rules.length})</span>
                        </button>
                    </td>
                </tr>
            )}
            {!isCollapsed &&
                rules.map(rule => (
                    <tr key={rule.id} className="border-b last:border-b-0">
                        <td className="max-w-[240px] min-w-[160px] border-r px-3 py-2 font-medium" title={rule.description ?? undefined}>
                            <span className="line-clamp-1">{rule.title}</span>
                        </td>
                        {dimensions.map(dim => {
                            const key = `${rule.id}:${dim.id}`;
                            const cell = cells[key] ?? null;
                            return (
                                <td key={dim.id} className="px-1 py-1 text-center">
                                    <CellButton
                                        cell={cell}
                                        ruleId={rule.id}
                                        dimensionId={dim.id}
                                        onCellClick={onCellClick}
                                        onToggleCell={onToggleCell}
                                    />
                                </td>
                            );
                        })}
                    </tr>
                ))}
        </>
    );
}

interface CellButtonProps {
    cell: ViewCell | null;
    ruleId: string;
    dimensionId: string;
    onCellClick: (cell: ViewCell, ruleId: string, dimensionId: string) => void;
    onToggleCell: (ruleId: string, dimensionId: string) => void;
}

function CellButton({ cell, ruleId, dimensionId, onCellClick, onToggleCell }: CellButtonProps) {
    if (!cell) {
        return (
            <button
                type="button"
                onClick={() => onToggleCell(ruleId, dimensionId)}
                className={`inline-flex size-8 items-center justify-center rounded border text-xs font-medium transition-colors ${EMPTY_CELL}`}
                title="Click to create cell"
                aria-label="Create cell"
            >
                +
            </button>
        );
    }

    const codeCount = cell.codeCount ?? 0;
    const hasCode = codeCount > 0;

    return (
        <button
            type="button"
            onClick={() => onCellClick(cell, ruleId, dimensionId)}
            onContextMenu={e => {
                e.preventDefault();
                onToggleCell(ruleId, dimensionId);
            }}
            className={`relative inline-flex size-8 items-center justify-center rounded border text-xs font-bold tabular-nums transition-colors ${STATUS_COLORS[cell.status]}`}
            title={`${cell.status}${cell.requirementCount > 0 ? ` (${cell.requirementCount} req)` : ""}${
                hasCode ? ` · ${codeCount} code link${codeCount === 1 ? "" : "s"}` : ""
            } - Right-click to remove`}
            aria-label={`${cell.status}, ${cell.requirementCount} requirements${hasCode ? `, ${codeCount} code links` : ""}`}
        >
            {cell.status === "verified" ? (
                cell.requirementCount > 0 ? (
                    cell.requirementCount
                ) : (
                    <Check className="size-3.5" />
                )
            ) : cell.requirementCount > 0 ? (
                cell.requirementCount
            ) : (
                ""
            )}
            {hasCode && (
                <span
                    className="absolute -right-0.5 -bottom-0.5 inline-flex size-3 items-center justify-center rounded-full bg-sky-600 text-white"
                    aria-hidden="true"
                >
                    <Code2 className="size-2" />
                </span>
            )}
        </button>
    );
}

interface RuleGroup {
    category: string;
    rules: Rule[];
}

function groupByCategory(rules: Rule[]): RuleGroup[] {
    const map = new Map<string, Rule[]>();
    for (const rule of rules) {
        const cat = rule.category ?? "Uncategorized";
        const list = map.get(cat);
        if (list) {
            list.push(rule);
        } else {
            map.set(cat, [rule]);
        }
    }
    return Array.from(map.entries()).map(([category, rules]) => ({ category, rules }));
}
