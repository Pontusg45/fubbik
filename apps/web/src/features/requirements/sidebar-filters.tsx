import { FolderOpen, Search } from "lucide-react";

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
    useCases: Array<{ id: string; name: string; requirementCount: number }>;
    activeUseCaseId: string | null;
    onUseCaseClick: (id: string | null) => void;
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
    onUseCaseClick
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
                        {useCases.map(uc => (
                            <button
                                key={uc.id}
                                type="button"
                                onClick={() => onUseCaseClick(uc.id)}
                                className={`flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-sm transition-colors ${
                                    activeUseCaseId === uc.id
                                        ? "bg-indigo-500/10 font-medium text-indigo-600"
                                        : "text-muted-foreground hover:bg-muted hover:text-foreground"
                                }`}
                            >
                                <span className="flex items-center gap-1.5 truncate">
                                    <FolderOpen className="size-3.5 shrink-0" />
                                    {uc.name}
                                </span>
                                <span className="text-xs tabular-nums">{uc.requirementCount}</span>
                            </button>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
}
