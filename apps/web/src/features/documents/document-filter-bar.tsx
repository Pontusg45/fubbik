import { ChevronDown, ChevronRight, FolderOpen, Tag, X } from "lucide-react";
import { useState } from "react";

import { DocFilterPresets, type DocPresetFilters } from "./document-filter-presets";

interface DocumentFilterBarProps {
    allTags: string[];
    allTypes: string[];
    activeTags: string[];
    activeTypes: string[];
    groupBy: "folder" | "tag";
    totalCount: number;
    filteredCount: number;
    onToggleTag: (tag: string) => void;
    onToggleType: (type: string) => void;
    onSetGroupBy: (groupBy: "folder" | "tag") => void;
    onClearAll: () => void;
    onApplyPreset: (filters: DocPresetFilters) => void;
}

export function DocumentFilterBar({
    allTags,
    allTypes,
    activeTags,
    activeTypes,
    groupBy,
    totalCount,
    filteredCount,
    onToggleTag,
    onToggleType,
    onSetGroupBy,
    onClearAll,
    onApplyPreset,
}: DocumentFilterBarProps) {
    const [expanded, setExpanded] = useState(false);
    const hasActiveFilters = activeTags.length > 0 || activeTypes.length > 0;

    return (
        <div className="border-b border-border/50 px-3 py-2">
            {/* Group-by toggle */}
            <div className="mb-2 flex items-center gap-1.5">
                <button
                    type="button"
                    onClick={() => onSetGroupBy("folder")}
                    className={`flex items-center gap-1 rounded px-2 py-0.5 text-[11px] font-medium transition-colors ${
                        groupBy === "folder"
                            ? "bg-muted text-foreground"
                            : "text-muted-foreground hover:text-foreground"
                    }`}
                >
                    <FolderOpen className="size-3" />
                    Folder
                </button>
                <button
                    type="button"
                    onClick={() => onSetGroupBy("tag")}
                    className={`flex items-center gap-1 rounded px-2 py-0.5 text-[11px] font-medium transition-colors ${
                        groupBy === "tag"
                            ? "bg-muted text-foreground"
                            : "text-muted-foreground hover:text-foreground"
                    }`}
                >
                    <Tag className="size-3" />
                    Tag
                </button>
                <button
                    type="button"
                    onClick={() => setExpanded(e => !e)}
                    className="ml-auto flex items-center gap-0.5 text-[11px] text-muted-foreground hover:text-foreground transition-colors"
                >
                    Filters
                    {hasActiveFilters && (
                        <span className="ml-0.5 rounded-full bg-primary/20 px-1 text-[9px] text-primary">
                            {activeTags.length + activeTypes.length}
                        </span>
                    )}
                    {expanded ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                </button>
            </div>

            {/* Expanded filter area */}
            {expanded && (
                <div className="space-y-2 border-t border-border/30 pt-2">
                    {/* Tags */}
                    {allTags.length > 0 && (
                        <div>
                            <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Tags</div>
                            <div className="flex flex-wrap gap-1">
                                {allTags.map(tag => {
                                    const active = activeTags.includes(tag);
                                    return (
                                        <button
                                            key={tag}
                                            type="button"
                                            onClick={() => onToggleTag(tag)}
                                            className={`flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[11px] transition-colors ${
                                                active
                                                    ? "bg-primary/20 text-primary border border-primary/40"
                                                    : "bg-muted/50 text-muted-foreground hover:text-foreground border border-transparent"
                                            }`}
                                        >
                                            {tag}
                                            {active && <X className="size-2.5" />}
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    {/* Types */}
                    {allTypes.length > 0 && (
                        <div>
                            <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Type</div>
                            <div className="flex flex-wrap gap-1">
                                {allTypes.map(type => {
                                    const active = activeTypes.includes(type);
                                    return (
                                        <button
                                            key={type}
                                            type="button"
                                            onClick={() => onToggleType(type)}
                                            className={`flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[11px] transition-colors ${
                                                active
                                                    ? "bg-primary/20 text-primary border border-primary/40"
                                                    : "bg-muted/50 text-muted-foreground hover:text-foreground border border-transparent"
                                            }`}
                                        >
                                            {type}
                                            {active && <X className="size-2.5" />}
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    {/* Presets + clear */}
                    <div className="flex items-center justify-between pt-1">
                        <DocFilterPresets
                            currentFilters={{ activeTags, activeTypes, groupBy }}
                            onApplyPreset={onApplyPreset}
                        />
                        {hasActiveFilters && (
                            <button
                                type="button"
                                onClick={onClearAll}
                                className="text-[11px] text-muted-foreground hover:text-foreground transition-colors"
                            >
                                Clear all
                            </button>
                        )}
                    </div>

                    {/* Result count */}
                    {hasActiveFilters && (
                        <div className="text-[11px] text-muted-foreground">
                            Showing {filteredCount} of {totalCount} documents
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
