import {
    Bookmark,
    Bot,
    FolderPlus,
    SlidersHorizontal,
    X
} from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";

interface TagItem {
    id: string;
    name: string;
}

export interface ChunkFilterValues {
    type?: string;
    q?: string;
    sort?: string;
    tags?: string;
    size?: string;
    after?: string;
    enrichment?: string;
    minConnections?: string;
    origin?: string;
    reviewStatus?: string;
}

export interface ChunkFiltersPopoverProps {
    filters: ChunkFilterValues;
    activeFilterCount: number;
    hasActiveFilters: boolean;
    activeTags: string[];
    availableTags: TagItem[];
    codebaseId: string | null | undefined;
    onUpdateSearch: (params: Partial<ChunkFilterValues>) => void;
    onToggleTag: (tag: string) => void;
    onClearAllFilters: () => void;
    onShowSaveFilter: () => void;
    onCreateCollection: (name: string, filter: Record<string, string | undefined>, codebaseId?: string) => void;
}

const TYPES = ["note", "document", "reference", "schema", "checklist"];

export function ChunkFiltersPopover({
    filters,
    activeFilterCount,
    hasActiveFilters,
    activeTags,
    availableTags,
    codebaseId,
    onUpdateSearch,
    onToggleTag,
    onClearAllFilters,
    onShowSaveFilter,
    onCreateCollection,
}: ChunkFiltersPopoverProps) {
    const { type, q, sort, size, after, enrichment, minConnections, origin, reviewStatus } = filters;

    const [showSaveCollection, setShowSaveCollection] = useState(false);
    const [newCollectionName, setNewCollectionName] = useState("");

    function handleSaveCollection() {
        if (!newCollectionName.trim()) return;
        onCreateCollection(
            newCollectionName.trim(),
            {
                type,
                search: q,
                sort,
                tags: filters.tags,
                after,
                enrichment,
                minConnections,
                origin,
                reviewStatus,
            },
            codebaseId ?? undefined,
        );
        setNewCollectionName("");
        setShowSaveCollection(false);
    }

    return (
        <Popover>
            <PopoverTrigger className="hover:bg-muted inline-flex items-center gap-1.5 rounded-md border px-3 py-2 text-sm font-medium transition-colors">
                <SlidersHorizontal className="size-3.5" />
                Filters
                {activeFilterCount > 0 && (
                    <span className="bg-primary text-primary-foreground flex size-5 items-center justify-center rounded-full text-[10px] font-semibold">
                        {activeFilterCount}
                    </span>
                )}
            </PopoverTrigger>
            <PopoverContent side="bottom" align="end" className="w-80">
                <div className="space-y-4">
                    {/* Type */}
                    <div>
                        <p className="text-muted-foreground mb-2 text-xs font-medium tracking-wider uppercase">Type</p>
                        <div className="flex flex-wrap gap-1">
                            <Button
                                variant={!type ? "default" : "outline"}
                                size="sm"
                                onClick={() => onUpdateSearch({ type: undefined })}
                            >
                                All
                            </Button>
                            {TYPES.map(t => (
                                <Button
                                    key={t}
                                    variant={type === t ? "default" : "outline"}
                                    size="sm"
                                    onClick={() => onUpdateSearch({ type: type === t ? undefined : t })}
                                >
                                    {t}
                                </Button>
                            ))}
                        </div>
                    </div>

                    <Separator />

                    {/* Sort */}
                    <div>
                        <p className="text-muted-foreground mb-2 text-xs font-medium tracking-wider uppercase">Sort</p>
                        {q ? (
                            <span className="text-muted-foreground text-xs italic">Sorted by relevance (search active)</span>
                        ) : (
                            <select
                                value={sort ?? "newest"}
                                onChange={e => onUpdateSearch({ sort: e.target.value === "newest" ? undefined : e.target.value })}
                                className="bg-background w-full rounded-md border px-2 py-1.5 text-sm"
                            >
                                <option value="newest">Newest first</option>
                                <option value="oldest">Oldest first</option>
                                <option value="alpha">Alphabetical (A-Z)</option>
                                <option value="updated">Recently updated</option>
                            </select>
                        )}
                    </div>

                    <Separator />

                    {/* Tags */}
                    {availableTags.length > 0 && (
                        <>
                            <div>
                                <p className="text-muted-foreground mb-2 text-xs font-medium tracking-wider uppercase">Tags</p>
                                <div className="flex flex-wrap gap-1">
                                    {availableTags.map(t => (
                                        <Badge
                                            key={t.id}
                                            variant={activeTags.includes(t.name) ? "default" : "outline"}
                                            size="sm"
                                            className="cursor-pointer text-[10px]"
                                            onClick={() => onToggleTag(t.name)}
                                        >
                                            {t.name}
                                        </Badge>
                                    ))}
                                </div>
                            </div>
                            <Separator />
                        </>
                    )}

                    {/* Date range */}
                    <div>
                        <p className="text-muted-foreground mb-2 text-xs font-medium tracking-wider uppercase">Date range</p>
                        <div className="flex gap-1">
                            {[
                                { label: "7 days", value: "7" },
                                { label: "30 days", value: "30" },
                                { label: "90 days", value: "90" }
                            ].map(opt => (
                                <Button
                                    key={opt.value}
                                    variant={after === opt.value ? "default" : "outline"}
                                    size="sm"
                                    onClick={() => onUpdateSearch({ after: after === opt.value ? undefined : opt.value })}
                                >
                                    {opt.label}
                                </Button>
                            ))}
                        </div>
                    </div>

                    <Separator />

                    {/* Content size */}
                    <div>
                        <p className="text-muted-foreground mb-2 text-xs font-medium tracking-wider uppercase">Content size</p>
                        <div className="flex gap-1">
                            {(["good", "moderate", "warning", "critical"] as const).map(level => {
                                const config: Record<string, { color: string; label: string }> = {
                                    good: { color: "#22c55e", label: "Good" },
                                    moderate: { color: "#f59e0b", label: "Moderate" },
                                    warning: { color: "#f97316", label: "Warning" },
                                    critical: { color: "#ef4444", label: "Critical" }
                                };
                                const c = config[level]!;
                                return (
                                    <button
                                        key={level}
                                        onClick={() => onUpdateSearch({ size: size === level ? undefined : level })}
                                        className={`rounded-full px-2.5 py-1 text-[10px] font-medium text-white transition-opacity ${size === level ? "ring-2 ring-offset-1" : "opacity-50 hover:opacity-80"}`}
                                        style={{ backgroundColor: c.color }}
                                    >
                                        {c.label}
                                    </button>
                                );
                            })}
                        </div>
                    </div>

                    <Separator />

                    {/* Enrichment */}
                    <div>
                        <p className="text-muted-foreground mb-2 text-xs font-medium tracking-wider uppercase">Enrichment</p>
                        <div className="flex gap-1">
                            <Badge
                                variant={enrichment === "missing" ? "default" : "outline"}
                                size="sm"
                                className="cursor-pointer"
                                onClick={() => onUpdateSearch({ enrichment: enrichment === "missing" ? undefined : "missing" })}
                            >
                                Needs enrichment
                            </Badge>
                            <Badge
                                variant={enrichment === "complete" ? "default" : "outline"}
                                size="sm"
                                className="cursor-pointer"
                                onClick={() => onUpdateSearch({ enrichment: enrichment === "complete" ? undefined : "complete" })}
                            >
                                Enriched
                            </Badge>
                        </div>
                    </div>

                    <Separator />

                    {/* Connections */}
                    <div>
                        <p className="text-muted-foreground mb-2 text-xs font-medium tracking-wider uppercase">Min connections</p>
                        <div className="flex gap-1">
                            {["1", "3", "5"].map(n => (
                                <Button
                                    key={n}
                                    variant={minConnections === n ? "default" : "outline"}
                                    size="sm"
                                    onClick={() => onUpdateSearch({ minConnections: minConnections === n ? undefined : n })}
                                >
                                    {n}+
                                </Button>
                            ))}
                        </div>
                    </div>

                    {/* Origin */}
                    <div>
                        <p className="text-muted-foreground mb-2 text-xs font-medium tracking-wider uppercase">Origin</p>
                        <div className="flex gap-1">
                            <Button
                                variant={!origin ? "default" : "outline"}
                                size="sm"
                                onClick={() => onUpdateSearch({ origin: undefined })}
                            >
                                All
                            </Button>
                            <Button
                                variant={origin === "human" ? "default" : "outline"}
                                size="sm"
                                onClick={() => onUpdateSearch({ origin: origin === "human" ? undefined : "human" })}
                            >
                                Human
                            </Button>
                            <Button
                                variant={origin === "ai" ? "default" : "outline"}
                                size="sm"
                                onClick={() => onUpdateSearch({ origin: origin === "ai" ? undefined : "ai" })}
                            >
                                <Bot className="mr-1 size-3" />
                                AI
                            </Button>
                        </div>
                    </div>

                    <Separator />

                    {/* Review Status */}
                    <div>
                        <p className="text-muted-foreground mb-2 text-xs font-medium tracking-wider uppercase">Review status</p>
                        <div className="flex gap-1">
                            <Button
                                variant={!reviewStatus ? "default" : "outline"}
                                size="sm"
                                onClick={() => onUpdateSearch({ reviewStatus: undefined })}
                            >
                                All
                            </Button>
                            {(["draft", "reviewed", "approved"] as const).map(s => (
                                <Button
                                    key={s}
                                    variant={reviewStatus === s ? "default" : "outline"}
                                    size="sm"
                                    onClick={() => onUpdateSearch({ reviewStatus: reviewStatus === s ? undefined : s })}
                                >
                                    {s.charAt(0).toUpperCase() + s.slice(1)}
                                </Button>
                            ))}
                        </div>
                    </div>

                    <Separator />

                    {/* Save / Clear */}
                    {hasActiveFilters && (
                        <>
                            <Separator />
                            <div className="flex gap-2">
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="flex-1"
                                    onClick={() => onShowSaveFilter()}
                                >
                                    <Bookmark className="size-3.5" />
                                    Save preset
                                </Button>
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="flex-1"
                                    onClick={() => {
                                        setShowSaveCollection(true);
                                    }}
                                >
                                    <FolderPlus className="size-3.5" />
                                    Save collection
                                </Button>
                            </div>
                            {showSaveCollection && (
                                <div className="flex gap-1.5">
                                    <input
                                        type="text"
                                        value={newCollectionName}
                                        onChange={e => setNewCollectionName(e.target.value)}
                                        onKeyDown={e => {
                                            if (e.key === "Enter") handleSaveCollection();
                                        }}
                                        placeholder="Collection name..."
                                        className="bg-background flex-1 rounded-md border px-2 py-1 text-sm"
                                        autoFocus
                                    />
                                    <Button
                                        size="sm"
                                        disabled={!newCollectionName.trim()}
                                        onClick={handleSaveCollection}
                                    >
                                        Save
                                    </Button>
                                </div>
                            )}
                            <Button variant="ghost" size="sm" className="w-full" onClick={onClearAllFilters}>
                                <X className="size-3.5" />
                                Clear all
                            </Button>
                        </>
                    )}
                </div>
            </PopoverContent>
        </Popover>
    );
}
