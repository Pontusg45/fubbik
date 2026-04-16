import { Filter, SlidersHorizontal } from "lucide-react";
import { useState } from "react";

import { FilterPresets, type FilterPresetFilters } from "@/features/graph/filter-presets";
import {
    GraphFilterForm,
    type GraphFilterPreviewData,
    type GraphFilterValues
} from "@/features/graph/graph-filter-form";

/**
 * Top-left inline filter panel.
 *
 * Shows the same filter fields as the modal dialog (tags, types, focus+depth,
 * group-by) — edits here commit immediately to the graph prefilter. The modal
 * dialog is a bigger-screen variant of the same form; both are thin wrappers
 * around GraphFilterForm so they can't drift.
 *
 * The view-only controls (show ungrouped, animate edges, saved presets) live
 * only here since they don't affect which chunks are fetched.
 */
interface GraphFiltersProps {
    filter: GraphFilterValues;
    onFilterChange: (values: GraphFilterValues) => void;
    previewData?: GraphFilterPreviewData;
    initialFocusTitle?: string;

    // View toggles (local-only, not part of prefilter)
    edgeAnimated?: boolean;
    onToggleEdgeAnimated?: () => void;
    showUngrouped?: boolean;
    onToggleUngrouped?: () => void;
    hasActiveGrouping?: boolean;

    // Saved filter presets (apply all three legacy dimensions at once)
    onApplyPreset?: (filters: FilterPresetFilters) => void;
    activeTypes?: Set<string>;
    activeRelations?: Set<string>;
    activeTagTypeIds?: Set<string>;
}

export function GraphFilters({
    filter,
    onFilterChange,
    previewData,
    initialFocusTitle,
    edgeAnimated,
    onToggleEdgeAnimated,
    showUngrouped,
    onToggleUngrouped,
    hasActiveGrouping,
    onApplyPreset,
    activeTypes,
    activeRelations,
    activeTagTypeIds
}: GraphFiltersProps) {
    const [collapsed, setCollapsed] = useState(false);

    return (
        <div className="bg-background/80 absolute top-4 left-4 z-10 w-[260px] max-w-[calc(100vw-2rem)] rounded-lg border p-3 backdrop-blur-sm max-md:w-[200px]">
            <button
                type="button"
                onClick={() => setCollapsed(c => !c)}
                className="mb-2 flex w-full items-center gap-1.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground hover:text-foreground"
            >
                <Filter className="size-3" />
                <span>Filter</span>
                <SlidersHorizontal className="ml-auto size-3 opacity-60" />
            </button>

            {!collapsed && (
                <>
                    <GraphFilterForm
                        values={filter}
                        onChange={onFilterChange}
                        previewData={previewData}
                        compact
                        initialFocusTitle={initialFocusTitle}
                    />

                    {onToggleUngrouped && hasActiveGrouping && (
                        <div className="mt-3 border-t pt-2">
                            <label className="flex cursor-pointer items-center gap-2 text-[10px]">
                                <input
                                    type="checkbox"
                                    checked={showUngrouped}
                                    onChange={onToggleUngrouped}
                                    className="accent-primary size-3"
                                />
                                <span className="text-muted-foreground font-medium uppercase">Show ungrouped</span>
                            </label>
                        </div>
                    )}

                    {onToggleEdgeAnimated && (
                        <div className="mt-2">
                            <label className="flex cursor-pointer items-center gap-2 text-[10px]">
                                <input
                                    type="checkbox"
                                    checked={edgeAnimated}
                                    onChange={onToggleEdgeAnimated}
                                    className="accent-primary size-3"
                                />
                                <span className="text-muted-foreground font-medium uppercase">Animate edges</span>
                            </label>
                        </div>
                    )}

                    {onApplyPreset && (
                        <div className="mt-3 border-t pt-2">
                            <FilterPresets
                                currentFilters={{
                                    activeTypes: [...(activeTypes ?? [])],
                                    activeRelations: [...(activeRelations ?? [])],
                                    activeTagTypeIds: [...(activeTagTypeIds ?? [])]
                                }}
                                onApplyPreset={onApplyPreset}
                            />
                        </div>
                    )}
                </>
            )}
        </div>
    );
}
