import { ChevronDown, ChevronRight, FolderOpen, Search } from "lucide-react";

import { useState } from "react";

import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";

interface SidebarFiltersProps {
    search: string;
    onSearchChange: (value: string) => void;
    statusFilters: string[];
    onStatusFiltersChange: (statuses: string[]) => void;
    priorityFilters: string[];
    onPriorityFiltersChange: (priorities: string[]) => void;
    originFilter: string;
    onOriginFilterChange: (origin: string) => void;
    useCases: Array<{ id: string; name: string; requirementCount: number; parentId: string | null }>;
    activeUseCaseId: string | null;
    onUseCaseClick: (id: string | null) => void;
    statusCounts?: Record<string, number>;
}

const STATUS_OPTIONS = [
    { value: "passing", label: "Passing" },
    { value: "failing", label: "Failing" },
    { value: "untested", label: "Untested" }
];

const PRIORITY_OPTIONS = [
    { value: "must", label: "Must" },
    { value: "should", label: "Should" },
    { value: "could", label: "Could" },
    { value: "wont", label: "Won't" }
];

const ORIGIN_OPTIONS = [
    { value: "", label: "All" },
    { value: "human", label: "Human" },
    { value: "ai", label: "AI" }
];

function toggleFilter(filters: string[], value: string): string[] {
    return filters.includes(value)
        ? filters.filter(f => f !== value)
        : [...filters, value];
}

export function SidebarFilters({
    search,
    onSearchChange,
    statusFilters,
    onStatusFiltersChange,
    priorityFilters,
    onPriorityFiltersChange,
    originFilter,
    onOriginFilterChange,
    useCases,
    activeUseCaseId,
    onUseCaseClick,
    statusCounts
}: SidebarFiltersProps) {
    return (
        <div className="space-y-6">
            {/* Search */}
            <div className="relative">
                <Search className="text-muted-foreground absolute left-2.5 top-2.5 size-4" />
                <Input
                    value={search}
                    onChange={e => onSearchChange(e.target.value)}
                    placeholder="Search requirements..."
                    className="pl-9"
                />
            </div>

            {/* Status */}
            <div>
                <h4 className="text-muted-foreground mb-2 text-xs font-semibold uppercase tracking-wider">
                    Status
                </h4>
                <div className="space-y-2">
                    {STATUS_OPTIONS.map(opt => (
                        <label key={opt.value} className="flex cursor-pointer items-center gap-2 text-sm">
                            <Checkbox
                                checked={statusFilters.includes(opt.value)}
                                onCheckedChange={() =>
                                    onStatusFiltersChange(toggleFilter(statusFilters, opt.value))
                                }
                            />
                            {opt.label}
                            {statusCounts?.[opt.value] != null && (
                                <span className="text-muted-foreground ml-auto text-xs">{statusCounts[opt.value]}</span>
                            )}
                        </label>
                    ))}
                </div>
            </div>

            {/* Priority */}
            <div>
                <h4 className="text-muted-foreground mb-2 text-xs font-semibold uppercase tracking-wider">
                    Priority
                </h4>
                <div className="space-y-2">
                    {PRIORITY_OPTIONS.map(opt => (
                        <label key={opt.value} className="flex cursor-pointer items-center gap-2 text-sm">
                            <Checkbox
                                checked={priorityFilters.includes(opt.value)}
                                onCheckedChange={() =>
                                    onPriorityFiltersChange(toggleFilter(priorityFilters, opt.value))
                                }
                            />
                            {opt.label}
                        </label>
                    ))}
                </div>
            </div>

            {/* Origin */}
            <div>
                <h4 className="text-muted-foreground mb-2 text-xs font-semibold uppercase tracking-wider">
                    Origin
                </h4>
                <div className="flex gap-1">
                    {ORIGIN_OPTIONS.map(opt => (
                        <button
                            key={opt.value}
                            type="button"
                            onClick={() => onOriginFilterChange(opt.value)}
                            className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
                                originFilter === opt.value
                                    ? "bg-primary text-primary-foreground"
                                    : "bg-muted text-muted-foreground hover:text-foreground"
                            }`}
                        >
                            {opt.label}
                        </button>
                    ))}
                </div>
            </div>

            {/* Use Cases */}
            {useCases.length > 0 && (
                <UseCaseTree
                    useCases={useCases}
                    activeUseCaseId={activeUseCaseId}
                    onUseCaseClick={onUseCaseClick}
                />
            )}
        </div>
    );
}

function UseCaseTree({
    useCases,
    activeUseCaseId,
    onUseCaseClick
}: {
    useCases: SidebarFiltersProps["useCases"];
    activeUseCaseId: string | null;
    onUseCaseClick: (id: string | null) => void;
}) {
    const [expanded, setExpanded] = useState<Set<string>>(new Set());

    const parents = useCases.filter(uc => uc.parentId === null);
    const childrenMap = new Map<string, typeof useCases>();
    for (const uc of useCases) {
        if (uc.parentId) {
            const list = childrenMap.get(uc.parentId) ?? [];
            list.push(uc);
            childrenMap.set(uc.parentId, list);
        }
    }

    function toggleExpanded(id: string) {
        setExpanded(prev => {
            const next = new Set(prev);
            if (next.has(id)) {
                next.delete(id);
            } else {
                next.add(id);
            }
            return next;
        });
    }

    function getTotalCount(uc: { id: string; requirementCount: number }) {
        const children = childrenMap.get(uc.id);
        if (!children) return uc.requirementCount;
        return uc.requirementCount + children.reduce((sum, c) => sum + c.requirementCount, 0);
    }

    return (
        <div>
            <h4 className="text-muted-foreground mb-2 text-xs font-semibold uppercase tracking-wider">
                Use Cases
            </h4>
            <div className="space-y-0.5">
                <button
                    type="button"
                    onClick={() => onUseCaseClick(null)}
                    className={`flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-sm transition-colors ${
                        activeUseCaseId === null
                            ? "bg-indigo-500/10 font-medium text-indigo-600"
                            : "text-muted-foreground hover:bg-muted hover:text-foreground"
                    }`}
                >
                    <span>All</span>
                </button>
                {parents.map(uc => {
                    const children = childrenMap.get(uc.id);
                    const hasChildren = !!children && children.length > 0;
                    const isExpanded = expanded.has(uc.id);

                    return (
                        <div key={uc.id}>
                            <div className="flex items-center">
                                {hasChildren ? (
                                    <button
                                        type="button"
                                        onClick={() => toggleExpanded(uc.id)}
                                        className="text-muted-foreground hover:text-foreground flex shrink-0 items-center justify-center rounded p-0.5"
                                    >
                                        {isExpanded ? (
                                            <ChevronDown className="size-3.5" />
                                        ) : (
                                            <ChevronRight className="size-3.5" />
                                        )}
                                    </button>
                                ) : (
                                    <span className="w-4.5 shrink-0" />
                                )}
                                <button
                                    type="button"
                                    onClick={() => onUseCaseClick(uc.id)}
                                    className={`flex min-w-0 flex-1 items-center justify-between rounded-md px-2 py-1.5 text-left text-sm transition-colors ${
                                        activeUseCaseId === uc.id
                                            ? "bg-indigo-500/10 font-medium text-indigo-600"
                                            : "text-muted-foreground hover:bg-muted hover:text-foreground"
                                    }`}
                                >
                                    <span className="flex items-center gap-1.5 truncate">
                                        <FolderOpen className="size-3.5 shrink-0" />
                                        {uc.name}
                                    </span>
                                    <span className="text-xs tabular-nums">{getTotalCount(uc)}</span>
                                </button>
                            </div>
                            {hasChildren && isExpanded && (
                                <div className="space-y-0.5 pl-6">
                                    {children.map(child => (
                                        <button
                                            key={child.id}
                                            type="button"
                                            onClick={() => onUseCaseClick(child.id)}
                                            className={`flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-xs transition-colors ${
                                                activeUseCaseId === child.id
                                                    ? "bg-indigo-500/10 font-medium text-indigo-600"
                                                    : "text-muted-foreground hover:bg-muted hover:text-foreground"
                                            }`}
                                        >
                                            <span className="flex items-center gap-1.5 truncate">
                                                <FolderOpen className="size-3 shrink-0" />
                                                {child.name}
                                            </span>
                                            <span className="text-xs tabular-nums">{child.requirementCount}</span>
                                        </button>
                                    ))}
                                </div>
                            )}
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
