import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
    Columns3,
    Filter,
    FolderPlus,
    Globe,
    LayoutGrid,
    List,
    Search,
    X,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ChunkFilterPills } from "@/features/chunks/chunk-filter-pills";
import { ChunkFiltersPopover } from "@/features/chunks/chunk-filters-popover";
import { useCollections } from "@/features/chunks/use-collections";
import { useSavedFilters } from "@/features/chunks/use-saved-filters";
import type { ChunkSearchParams } from "@/features/chunks/use-chunk-filters";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface ChunksToolbarProps {
    // search / filter state
    searchInput: string;
    onSearchInputChange: (v: string) => void;
    type?: string;
    q?: string;
    sort?: string;
    tags?: string;
    size?: string;
    after?: string;
    enrichment?: string;
    minConnections?: string;
    group?: string;
    subGroup?: string;
    view?: string;
    origin?: string;
    reviewStatus?: string;
    allCodebases?: string;
    activeTags: string[];
    activeFilterCount: number;
    hasActiveFilters: boolean;
    isFederated: boolean;
    total: number;
    availableTags: { id: string; name: string; tagTypeId?: string | null }[];
    codebaseId?: string | null;
    // callbacks
    onUpdateSearch: (params: Partial<ChunkSearchParams>) => void;
    onToggleTag: (tag: string) => void;
    onClearAllFilters: () => void;
    onShowSaveFilter: () => void;
    searchInputRef: React.RefObject<HTMLInputElement | null>;
}

export function ChunksToolbar({
    searchInput,
    onSearchInputChange,
    type,
    q,
    sort,
    tags,
    size,
    after,
    enrichment,
    minConnections,
    group,
    subGroup,
    view,
    origin,
    reviewStatus,
    allCodebases,
    activeTags,
    activeFilterCount,
    hasActiveFilters,
    isFederated,
    total,
    availableTags,
    codebaseId,
    onUpdateSearch,
    onToggleTag,
    onClearAllFilters,
    onShowSaveFilter,
    searchInputRef,
}: ChunksToolbarProps) {
    const navTo = useNavigate();
    const { filters: savedFilters, deleteFilter } = useSavedFilters();
    const { collections, createCollection, deleteCollection: deleteCol } = useCollections();

    return (
        <>
            {/* Search bar + controls */}
            <div className="mb-4 flex flex-wrap items-center gap-2">
                <div className="relative flex-1">
                    <Search className="text-muted-foreground absolute top-1/2 left-3 size-4 -translate-y-1/2" />
                    <input
                        ref={searchInputRef}
                        type="text"
                        value={searchInput}
                        onChange={e => onSearchInputChange(e.target.value)}
                        onKeyDown={e => {
                            if (e.key === "Enter") onUpdateSearch({ q: searchInput || undefined });
                        }}
                        placeholder="Search chunks..."
                        className="bg-background focus:ring-ring w-full rounded-md border py-2 pr-24 pl-9 text-sm focus:ring-2 focus:outline-none"
                    />
                    {total > 0 && (
                        <span className="text-muted-foreground absolute top-1/2 right-3 -translate-y-1/2 text-xs tabular-nums">
                            {total} {total === 1 ? "chunk" : "chunks"}
                        </span>
                    )}
                </div>

                {/* All codebases toggle */}
                <Button
                    variant={isFederated ? "default" : "outline"}
                    size="sm"
                    onClick={() => onUpdateSearch({ allCodebases: isFederated ? undefined : "true" })}
                    className="gap-1.5"
                    title="Search across all codebases"
                >
                    <Globe className="size-3.5" />
                    All
                </Button>

                {/* Filters popover */}
                <ChunkFiltersPopover
                    filters={{ type, q, sort, tags, size, after, enrichment, minConnections, origin, reviewStatus }}
                    activeFilterCount={activeFilterCount}
                    hasActiveFilters={hasActiveFilters}
                    activeTags={activeTags}
                    availableTags={availableTags}
                    codebaseId={codebaseId}
                    onUpdateSearch={onUpdateSearch}
                    onToggleTag={onToggleTag}
                    onClearAllFilters={onClearAllFilters}
                    onShowSaveFilter={onShowSaveFilter}
                    onCreateCollection={createCollection}
                />

                {/* View & grouping controls */}
                <TagTypeGroupSelect
                    value={group}
                    onChange={v => onUpdateSearch({ group: v ?? undefined, subGroup: undefined })}
                />
                {group && (
                    <SubGroupSelect
                        value={subGroup}
                        primaryGroup={group}
                        onChange={v => onUpdateSearch({ subGroup: v ?? undefined })}
                    />
                )}

                {collections.length > 0 && (
                    <Popover>
                        <PopoverTrigger className="hover:bg-muted inline-flex items-center gap-1.5 rounded-md border px-3 py-2 text-sm font-medium transition-colors">
                            <FolderPlus className="size-3.5" />
                            Collections
                        </PopoverTrigger>
                        <PopoverContent side="bottom" align="end" className="w-56">
                            <div className="space-y-1">
                                {collections.map(c => (
                                    <div key={c.id} className="flex items-center justify-between gap-1 rounded px-2 py-1.5 text-sm">
                                        <button
                                            className="hover:text-foreground text-muted-foreground flex-1 truncate text-left"
                                            onClick={() => {
                                                const f = c.filter as Record<string, string | undefined>;
                                                navTo({
                                                    from: "/chunks/",
                                                    search: {
                                                        type: f.type,
                                                        q: f.search,
                                                        sort: f.sort,
                                                        tags: f.tags,
                                                        after: f.after,
                                                        enrichment: f.enrichment,
                                                        minConnections: f.minConnections,
                                                        origin: f.origin,
                                                        reviewStatus: f.reviewStatus,
                                                        size: undefined,
                                                        group,
                                                        collection: undefined,
                                                        view
                                                    }
                                                });
                                            }}
                                        >
                                            {c.name}
                                        </button>
                                        <button
                                            onClick={() => deleteCol(c.id)}
                                            className="text-muted-foreground hover:text-destructive shrink-0"
                                        >
                                            <X className="size-3" />
                                        </button>
                                    </div>
                                ))}
                            </div>
                        </PopoverContent>
                    </Popover>
                )}

                <div className="flex rounded-md border">
                    <button onClick={() => onUpdateSearch({ view: undefined })} className={`px-2 py-1.5 text-xs ${!view ? "bg-muted" : ""}`}>
                        <List className="size-3.5" />
                    </button>
                    <button
                        onClick={() => onUpdateSearch({ view: "grid" })}
                        className={`px-2 py-1.5 text-xs ${view === "grid" ? "bg-muted" : ""}`}
                    >
                        <LayoutGrid className="size-3.5" />
                    </button>
                    <button
                        onClick={() => onUpdateSearch({ view: "kanban" })}
                        className={`px-2 py-1.5 text-xs ${view === "kanban" ? "bg-muted" : ""}`}
                    >
                        <Columns3 className="size-3.5" />
                    </button>
                </div>
            </div>

            {/* Active filter pills + clear all */}
            <ChunkFilterPills
                type={type}
                q={q}
                tags={tags}
                after={after}
                enrichment={enrichment}
                minConnections={minConnections}
                origin={origin}
                reviewStatus={reviewStatus}
                allCodebases={allCodebases}
                activeTags={activeTags}
                onRemoveFilter={(key) => {
                    if (key.startsWith("tag:")) {
                        const tagToRemove = key.slice(4);
                        const remaining = activeTags.filter(t => t !== tagToRemove);
                        onUpdateSearch({ tags: remaining.length > 0 ? remaining.join(",") : undefined });
                    } else if (key === "q") {
                        onSearchInputChange("");
                        onUpdateSearch({ q: undefined });
                    } else {
                        onUpdateSearch({ [key]: undefined } as Partial<ChunkSearchParams>);
                    }
                }}
                onClearAll={onClearAllFilters}
            />

            {/* Saved filter presets */}
            {savedFilters.length > 0 && (
                <div className="mb-3 flex flex-wrap items-center gap-1.5">
                    <Filter className="text-muted-foreground size-3" />
                    {savedFilters.map(f => (
                        <Badge
                            key={f.name}
                            variant="outline"
                            className="cursor-pointer gap-1"
                            onClick={() => navTo({ from: "/chunks/", search: { ...f.params } })}
                        >
                            {f.name}
                            <button
                                onClick={e => {
                                    e.stopPropagation();
                                    deleteFilter(f.name);
                                }}
                            >
                                <X className="size-2.5" />
                            </button>
                        </Badge>
                    ))}
                </div>
            )}
        </>
    );
}

export function SubGroupSelect({
    value,
    primaryGroup,
    onChange,
}: {
    value?: string;
    primaryGroup: string;
    onChange: (v: string | undefined) => void;
}) {
    const tagTypesQuery = useQuery({
        queryKey: ["tag-types"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api["tag-types"].get());
            } catch {
                return [];
            }
        },
        staleTime: 60_000
    });
    const tagTypes = tagTypesQuery.data ?? [];
    const isPrimaryTagType = primaryGroup.startsWith("tagtype:");

    return (
        <select
            value={value ?? ""}
            onChange={e => onChange(e.target.value || undefined)}
            className="bg-background rounded-md border px-2 py-2 text-sm"
        >
            <option value="">Then by...</option>
            {primaryGroup !== "type" && <option value="type">Type</option>}
            {primaryGroup !== "status" && <option value="status">Status</option>}
            {primaryGroup !== "origin" && <option value="origin">Origin</option>}
            {primaryGroup !== "freshness" && <option value="freshness">Freshness</option>}
            {tagTypes
                .filter(tt => !isPrimaryTagType || primaryGroup !== `tagtype:${tt.id}`)
                .map(tt => (
                    <option key={tt.id} value={`tagtype:${tt.id}`}>
                        Tag: {tt.name}
                    </option>
                ))}
        </select>
    );
}

export function TagTypeGroupSelect({ value, onChange }: { value?: string; onChange: (v: string | undefined) => void }) {
    const tagTypesQuery = useQuery({
        queryKey: ["tag-types"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api["tag-types"].get());
            } catch {
                return [];
            }
        },
        staleTime: 60_000
    });

    const tagTypes = tagTypesQuery.data ?? [];

    return (
        <select
            value={value ?? ""}
            onChange={e => onChange(e.target.value || undefined)}
            className="bg-background rounded-md border px-2 py-2 text-sm"
        >
            <option value="">No grouping</option>
            <option value="type">Group by type</option>
            {tagTypes.map(tt => (
                <option key={tt.id} value={`tagtype:${tt.id}`}>
                    Group by tag: {tt.name}
                </option>
            ))}
            <option value="status">Group by status</option>
            <option value="origin">Group by origin</option>
            <option value="freshness">Group by freshness</option>
        </select>
    );
}
