import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import {
    Bookmark,
    ChevronRight,
    Clock,
    Columns3,
    FileText,
    Filter,
    FolderPlus,
    List,
    Pin,
    Plus,
    Search,
    SlidersHorizontal,
    Trash2,
    X
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { getChunkSize } from "@/features/chunks/chunk-size";
import { KanbanView } from "@/features/chunks/kanban-view";
import { useCollections } from "@/features/chunks/use-collections";
import { usePinnedChunks } from "@/features/chunks/use-pinned-chunks";
import { useSavedFilters } from "@/features/chunks/use-saved-filters";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/chunks/")({
    component: ChunksList,
    validateSearch: (search: Record<string, unknown>) => ({
        type: (search.type as string) || undefined,
        q: (search.q as string) || undefined,
        page: Number(search.page) || 1,
        sort: (search.sort as string) || undefined,
        tags: (search.tags as string) || undefined,
        size: (search.size as string) || undefined,
        after: (search.after as string) || undefined,
        enrichment: (search.enrichment as string) || undefined,
        minConnections: (search.minConnections as string) || undefined,
        group: (search.group as string) || undefined,
        collection: (search.collection as string) || undefined,
        view: (search.view as string) || undefined
    }),
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest access
        }
        return { session };
    }
});

function ChunksList() {
    const navigate = useNavigate({ from: "/chunks/" });
    const navTo = useNavigate();
    const { type, q, page, sort, tags, size, after, enrichment, minConnections, group, collection, view } = Route.useSearch();
    const queryClient = useQueryClient();
    const [searchInput, setSearchInput] = useState(q ?? "");
    const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
    const { filters: savedFilters, saveFilter, deleteFilter } = useSavedFilters();
    const { codebaseId } = useActiveCodebase();
    const limit = 20;
    const offset = ((page ?? 1) - 1) * limit;

    const activeFilterCount = [tags, size, after, enrichment, minConnections].filter(Boolean).length;
    const hasActiveFilters = !!(type || q || sort || tags || size || after || enrichment || minConnections);

    const chunksQuery = useQuery({
        queryKey: ["chunks-list", type, q, page, sort, tags, after, enrichment, minConnections, codebaseId],
        queryFn: async () => {
            try {
                return unwrapEden(
                    await api.api.chunks.get({
                        query: {
                            type,
                            search: q,
                            sort: sort as "newest" | "oldest" | "alpha" | "updated" | undefined,
                            tags,
                            after,
                            enrichment: enrichment as "missing" | "complete" | undefined,
                            minConnections,
                            limit: String(limit),
                            offset: String(offset),
                            ...(codebaseId === "global" ? { global: "true" } : codebaseId ? { codebaseId } : {})
                        }
                    })
                );
            } catch {
                return null;
            }
        }
    });

    const tagsQuery = useQuery({
        queryKey: ["tags"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.tags.get());
            } catch {
                return [];
            }
        }
    });

    const activeTags = tags ? tags.split(",") : [];

    function toggleTag(tag: string) {
        const current = activeTags;
        const next = current.includes(tag) ? current.filter(t => t !== tag) : [...current, tag];
        updateSearch({ tags: next.length > 0 ? next.join(",") : undefined });
    }

    const chunks = chunksQuery.data?.chunks ?? [];
    const { pinnedIds, togglePin, isPinned } = usePinnedChunks();
    const { collections, createCollection, addToCollection } = useCollections();

    const processedChunks = useMemo(() => {
        const source = chunksQuery.data?.chunks ?? [];
        const filtered = size ? source.filter(c => getChunkSize(c.content).level === size) : source;
        const pinnedSet = new Set(pinnedIds);
        return [...filtered].sort((a, b) => {
            const aPinned = pinnedSet.has(a.id) ? 0 : 1;
            const bPinned = pinnedSet.has(b.id) ? 0 : 1;
            return aPinned - bPinned;
        });
        // eslint-disable-next-line react-hooks/exhaustive-deps -- chunksQuery.data is stable per fetch
    }, [chunksQuery.data, size, pinnedIds]);

    const collectionFilteredChunks = useMemo(() => {
        if (!collection) return processedChunks;
        const col = collections.find(c => c.id === collection);
        if (!col) return processedChunks;
        const idSet = new Set(col.chunkIds);
        return processedChunks.filter(c => idSet.has(c.id));
    }, [processedChunks, collection, collections]);

    const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
    function toggleCollapsed(key: string) {
        setCollapsed(prev => {
            const next = new Set(prev);
            if (next.has(key)) next.delete(key);
            else next.add(key);
            return next;
        });
    }

    const groupedChunks = useMemo(() => {
        if (group === "type") {
            const groups = new Map<string, typeof collectionFilteredChunks>();
            for (const c of collectionFilteredChunks) {
                const existing = groups.get(c.type) ?? [];
                existing.push(c);
                groups.set(c.type, existing);
            }
            return groups;
        }
        if (group === "tag") {
            const groups = new Map<string, typeof collectionFilteredChunks>();
            for (const c of collectionFilteredChunks) {
                const chunkTags: string[] = [];
                if (chunkTags.length === 0) {
                    const existing = groups.get("untagged") ?? [];
                    existing.push(c);
                    groups.set("untagged", existing);
                }
                for (const tag of chunkTags) {
                    const existing = groups.get(tag) ?? [];
                    existing.push(c);
                    groups.set(tag, existing);
                }
            }
            return groups;
        }
        return null;
    }, [collectionFilteredChunks, group]);

    const chunksRef = useRef(chunks);
    chunksRef.current = chunks;
    const [selectedIndex, setSelectedIndex] = useState(-1);

    useEffect(() => {
        setSelectedIndex(-1);
    }, [chunksQuery.dataUpdatedAt]);

    useEffect(() => {
        function handleKeyDown(e: KeyboardEvent) {
            if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
            const list = chunksRef.current;
            switch (e.key) {
                case "j":
                    setSelectedIndex(i => Math.min(i + 1, list.length - 1));
                    break;
                case "k":
                    setSelectedIndex(i => Math.max(i - 1, 0));
                    break;
                case "Enter":
                    if (selectedIndex >= 0 && selectedIndex < list.length) {
                        navTo({ to: "/chunks/$chunkId", params: { chunkId: list[selectedIndex]!.id } });
                    }
                    break;
                case "n":
                    navTo({ to: "/chunks/new" });
                    break;
                case "e":
                    if (selectedIndex >= 0 && selectedIndex < list.length) {
                        navTo({ to: "/chunks/$chunkId", params: { chunkId: list[selectedIndex]!.id } });
                    }
                    break;
            }
        }
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [selectedIndex, navTo]);

    const total = chunksQuery.data?.total ?? 0;
    const totalPages = Math.ceil(total / limit);

    const types = ["note", "document", "reference", "schema", "checklist"];

    function updateSearch(
        params: Partial<{
            type: string;
            q: string;
            page: number;
            sort: string;
            tags: string;
            size: string;
            after: string;
            enrichment: string;
            minConnections: string;
            group: string;
            collection: string;
            view: string;
        }>
    ) {
        navigate({
            search: {
                type: params.type !== undefined ? params.type : type,
                q: params.q !== undefined ? params.q : q,
                page: params.page ?? 1,
                sort: params.sort !== undefined ? params.sort : sort,
                tags: params.tags !== undefined ? params.tags : tags,
                size: params.size !== undefined ? params.size : size,
                after: params.after !== undefined ? params.after : after,
                enrichment: params.enrichment !== undefined ? params.enrichment : enrichment,
                minConnections: params.minConnections !== undefined ? params.minConnections : minConnections,
                group: params.group !== undefined ? params.group : group,
                collection: params.collection !== undefined ? params.collection : collection,
                view: params.view !== undefined ? params.view : view
            }
        });
    }

    function clearAllFilters() {
        navigate({
            search: {
                type: undefined,
                q: undefined,
                page: 1,
                sort: undefined,
                tags: undefined,
                size: undefined,
                after: undefined,
                enrichment: undefined,
                minConnections: undefined,
                group,
                collection,
                view
            }
        });
        setSearchInput("");
    }

    function toggleSelection(id: string) {
        setSelectedIds(prev => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return next;
        });
    }

    function toggleAll() {
        if (selectedIds.size === chunks.length) {
            setSelectedIds(new Set());
        } else {
            setSelectedIds(new Set(chunks.map(c => c.id)));
        }
    }

    const bulkDeleteMutation = useMutation({
        mutationFn: async (ids: string[]) => {
            const { error } = await api.api.chunks.bulk.delete({ ids });
            if (error) throw new Error("Failed to delete chunks");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            setSelectedIds(new Set());
        }
    });

    function handleBulkDelete() {
        if (!confirm(`Delete ${selectedIds.size} chunks?`)) return;
        bulkDeleteMutation.mutate([...selectedIds]);
    }

    return (
        <div className="container mx-auto max-w-5xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <div className="flex items-center gap-3">
                    {chunks.length > 0 && (
                        <Checkbox
                            checked={selectedIds.size === chunks.length && chunks.length > 0}
                            indeterminate={selectedIds.size > 0 && selectedIds.size < chunks.length}
                            onCheckedChange={toggleAll}
                        />
                    )}
                    <div>
                        <h1 className="text-2xl font-bold tracking-tight">Chunks</h1>
                        <p className="text-muted-foreground mt-1 text-xs">
                            <kbd className="bg-muted rounded px-1 py-0.5 font-mono text-[10px]">j</kbd>/
                            <kbd className="bg-muted rounded px-1 py-0.5 font-mono text-[10px]">k</kbd> navigate
                            <kbd className="bg-muted ml-2 rounded px-1 py-0.5 font-mono text-[10px]">Enter</kbd> open
                            <kbd className="bg-muted ml-2 rounded px-1 py-0.5 font-mono text-[10px]">n</kbd> new
                        </p>
                    </div>
                </div>
                <Button render={<Link to="/chunks/new" />}>
                    <Plus className="size-4" />
                    New Chunk
                </Button>
            </div>

            {/* Search bar + controls */}
            <div className="mb-4 flex flex-wrap items-center gap-2">
                <div className="relative flex-1">
                    <Search className="text-muted-foreground absolute top-1/2 left-3 size-4 -translate-y-1/2" />
                    <input
                        type="text"
                        value={searchInput}
                        onChange={e => setSearchInput(e.target.value)}
                        onKeyDown={e => {
                            if (e.key === "Enter") updateSearch({ q: searchInput || undefined });
                        }}
                        placeholder="Search chunks..."
                        className="bg-background focus:ring-ring w-full rounded-md border py-2 pr-3 pl-9 text-sm focus:ring-2 focus:outline-none"
                    />
                </div>

                {/* Filters popover */}
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
                                        onClick={() => updateSearch({ type: undefined })}
                                    >
                                        All
                                    </Button>
                                    {types.map(t => (
                                        <Button
                                            key={t}
                                            variant={type === t ? "default" : "outline"}
                                            size="sm"
                                            onClick={() => updateSearch({ type: type === t ? undefined : t })}
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
                                        onChange={e => updateSearch({ sort: e.target.value === "newest" ? undefined : e.target.value })}
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
                            {(tagsQuery.data ?? []).length > 0 && (
                                <>
                                    <div>
                                        <p className="text-muted-foreground mb-2 text-xs font-medium tracking-wider uppercase">Tags</p>
                                        <div className="flex flex-wrap gap-1">
                                            {(tagsQuery.data ?? []).map(t => (
                                                <Badge
                                                    key={t.id}
                                                    variant={activeTags.includes(t.name) ? "default" : "outline"}
                                                    size="sm"
                                                    className="cursor-pointer text-[10px]"
                                                    onClick={() => toggleTag(t.name)}
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
                                            onClick={() => updateSearch({ after: after === opt.value ? undefined : opt.value })}
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
                                                onClick={() => updateSearch({ size: size === level ? undefined : level })}
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
                                        onClick={() => updateSearch({ enrichment: enrichment === "missing" ? undefined : "missing" })}
                                    >
                                        Needs enrichment
                                    </Badge>
                                    <Badge
                                        variant={enrichment === "complete" ? "default" : "outline"}
                                        size="sm"
                                        className="cursor-pointer"
                                        onClick={() => updateSearch({ enrichment: enrichment === "complete" ? undefined : "complete" })}
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
                                            onClick={() => updateSearch({ minConnections: minConnections === n ? undefined : n })}
                                        >
                                            {n}+
                                        </Button>
                                    ))}
                                </div>
                            </div>

                            {/* Save / Clear */}
                            {hasActiveFilters && (
                                <>
                                    <Separator />
                                    <div className="flex gap-2">
                                        <Button
                                            variant="outline"
                                            size="sm"
                                            className="flex-1"
                                            onClick={() => {
                                                const name = prompt("Filter name:");
                                                if (name)
                                                    saveFilter(name, { type, q, sort, tags, size, after, enrichment, minConnections });
                                            }}
                                        >
                                            <Bookmark className="size-3.5" />
                                            Save preset
                                        </Button>
                                        <Button variant="ghost" size="sm" className="flex-1" onClick={clearAllFilters}>
                                            <X className="size-3.5" />
                                            Clear all
                                        </Button>
                                    </div>
                                </>
                            )}
                        </div>
                    </PopoverContent>
                </Popover>

                {/* View & grouping controls */}
                <select
                    value={group ?? ""}
                    onChange={e => updateSearch({ group: e.target.value || undefined })}
                    className="bg-background rounded-md border px-2 py-2 text-sm"
                >
                    <option value="">No grouping</option>
                    <option value="type">Group by type</option>
                    <option value="tag">Group by tag</option>
                </select>

                <select
                    value={collection ?? ""}
                    onChange={e => updateSearch({ collection: e.target.value || undefined })}
                    className="bg-background rounded-md border px-2 py-2 text-sm"
                >
                    <option value="">All chunks</option>
                    {collections.map(c => (
                        <option key={c.id} value={c.id}>
                            {c.name} ({c.chunkIds.length})
                        </option>
                    ))}
                </select>
                <Button
                    variant="outline"
                    size="sm"
                    onClick={() => {
                        const name = prompt("Collection name:");
                        if (name) createCollection(name);
                    }}
                >
                    <FolderPlus className="size-3.5" />
                </Button>

                <div className="flex rounded-md border">
                    <button onClick={() => updateSearch({ view: undefined })} className={`px-2 py-1.5 text-xs ${!view ? "bg-muted" : ""}`}>
                        <List className="size-3.5" />
                    </button>
                    <button
                        onClick={() => updateSearch({ view: "kanban" })}
                        className={`px-2 py-1.5 text-xs ${view === "kanban" ? "bg-muted" : ""}`}
                    >
                        <Columns3 className="size-3.5" />
                    </button>
                </div>
            </div>

            {/* Active filter pills + clear all (shown below search when filters active) */}
            {hasActiveFilters && (
                <div className="mb-4 flex flex-wrap items-center gap-1.5">
                    {type && (
                        <Badge variant="secondary" size="sm" className="gap-1">
                            Type: {type}
                            <button onClick={() => updateSearch({ type: undefined })}>
                                <X className="size-2.5" />
                            </button>
                        </Badge>
                    )}
                    {q && (
                        <Badge variant="secondary" size="sm" className="gap-1">
                            Search: {q}
                            <button
                                onClick={() => {
                                    setSearchInput("");
                                    updateSearch({ q: undefined });
                                }}
                            >
                                <X className="size-2.5" />
                            </button>
                        </Badge>
                    )}
                    {sort && (
                        <Badge variant="secondary" size="sm" className="gap-1">
                            Sort: {sort}
                            <button onClick={() => updateSearch({ sort: undefined })}>
                                <X className="size-2.5" />
                            </button>
                        </Badge>
                    )}
                    {activeTags.map(tag => (
                        <Badge key={tag} variant="secondary" size="sm" className="gap-1">
                            {tag}
                            <button onClick={() => toggleTag(tag)}>
                                <X className="size-2.5" />
                            </button>
                        </Badge>
                    ))}
                    {size && (
                        <Badge variant="secondary" size="sm" className="gap-1">
                            Size: {size}
                            <button onClick={() => updateSearch({ size: undefined })}>
                                <X className="size-2.5" />
                            </button>
                        </Badge>
                    )}
                    {after && (
                        <Badge variant="secondary" size="sm" className="gap-1">
                            Last {after}d
                            <button onClick={() => updateSearch({ after: undefined })}>
                                <X className="size-2.5" />
                            </button>
                        </Badge>
                    )}
                    {enrichment && (
                        <Badge variant="secondary" size="sm" className="gap-1">
                            {enrichment === "missing" ? "Needs enrichment" : "Enriched"}
                            <button onClick={() => updateSearch({ enrichment: undefined })}>
                                <X className="size-2.5" />
                            </button>
                        </Badge>
                    )}
                    {minConnections && (
                        <Badge variant="secondary" size="sm" className="gap-1">
                            {minConnections}+ connections
                            <button onClick={() => updateSearch({ minConnections: undefined })}>
                                <X className="size-2.5" />
                            </button>
                        </Badge>
                    )}
                    <button onClick={clearAllFilters} className="text-muted-foreground hover:text-foreground ml-1 text-xs underline">
                        Clear all
                    </button>
                </div>
            )}

            {/* Saved filter presets */}
            {savedFilters.length > 0 && (
                <div className="mb-3 flex flex-wrap items-center gap-1.5">
                    <Filter className="text-muted-foreground size-3" />
                    {savedFilters.map(f => (
                        <Badge
                            key={f.name}
                            variant="outline"
                            className="cursor-pointer gap-1"
                            onClick={() => navigate({ search: { ...f.params, page: 1 } as any })}
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

            {/* Results */}
            {view === "kanban" ? (
                <KanbanView chunks={collectionFilteredChunks} />
            ) : chunksQuery.isLoading ? (
                <Card>
                    <CardPanel className="p-8 text-center">
                        <p className="text-muted-foreground text-sm">Loading...</p>
                    </CardPanel>
                </Card>
            ) : collectionFilteredChunks.length === 0 ? (
                <Card>
                    <CardPanel className="p-8 text-center">
                        <p className="text-muted-foreground text-sm">No chunks found.</p>
                    </CardPanel>
                </Card>
            ) : groupedChunks ? (
                <div className="space-y-4">
                    {[...groupedChunks.entries()].map(([groupName, items]) => (
                        <div key={groupName}>
                            <button onClick={() => toggleCollapsed(groupName)} className="mb-2 flex items-center gap-2">
                                <ChevronRight className={`size-3 transition-transform ${!collapsed.has(groupName) && "rotate-90"}`} />
                                <Badge variant="secondary">{groupName}</Badge>
                                <span className="text-muted-foreground text-xs">({items.length})</span>
                            </button>
                            {!collapsed.has(groupName) && (
                                <Card>
                                    {items.map((chunk, i) => (
                                        <div key={chunk.id}>
                                            {i > 0 && <Separator />}
                                            <CardPanel className="hover:bg-muted/50 flex items-center gap-3 p-4 transition-colors">
                                                <Checkbox
                                                    checked={selectedIds.has(chunk.id)}
                                                    onCheckedChange={() => toggleSelection(chunk.id)}
                                                    onClick={(e: React.MouseEvent) => e.stopPropagation()}
                                                />
                                                <button
                                                    onClick={e => {
                                                        e.preventDefault();
                                                        e.stopPropagation();
                                                        togglePin(chunk.id);
                                                    }}
                                                    className="text-muted-foreground hover:text-foreground"
                                                >
                                                    <Pin className={`size-3 ${isPinned(chunk.id) ? "fill-current" : ""}`} />
                                                </button>
                                                <Link
                                                    to="/chunks/$chunkId"
                                                    params={{ chunkId: chunk.id }}
                                                    className="flex flex-1 items-center justify-between gap-4"
                                                >
                                                    <div className="min-w-0">
                                                        <p className="truncate text-sm font-medium">{chunk.title}</p>
                                                        <div className="mt-1 flex items-center gap-2">
                                                            <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                                                {chunk.type}
                                                            </Badge>
                                                            {([] as string[]).map(tag => (
                                                                <Badge key={tag} variant="outline" size="sm" className="text-[10px]">
                                                                    {tag}
                                                                </Badge>
                                                            ))}
                                                        </div>
                                                    </div>
                                                    <div className="flex shrink-0 items-center gap-3">
                                                        {(() => {
                                                            const chunkSize = getChunkSize(chunk.content);
                                                            return chunkSize.level !== "good" ? (
                                                                <span
                                                                    className="flex items-center gap-1 text-xs"
                                                                    style={{ color: chunkSize.color }}
                                                                >
                                                                    <FileText className="size-3" />
                                                                    {chunkSize.lines}L
                                                                </span>
                                                            ) : null;
                                                        })()}
                                                        <span className="text-muted-foreground flex items-center gap-1 text-xs">
                                                            <Clock className="size-3" />
                                                            {new Date(chunk.updatedAt).toLocaleDateString()}
                                                        </span>
                                                    </div>
                                                </Link>
                                            </CardPanel>
                                        </div>
                                    ))}
                                </Card>
                            )}
                        </div>
                    ))}
                </div>
            ) : (
                <Card>
                    {collectionFilteredChunks.map((chunk, i) => (
                        <div key={chunk.id}>
                            {i > 0 && <Separator />}
                            <CardPanel
                                className={`flex items-center gap-3 p-4 transition-colors ${selectedIndex === i ? "bg-muted/50 ring-primary/50 ring-2 ring-inset" : "hover:bg-muted/50"}`}
                            >
                                <Checkbox
                                    checked={selectedIds.has(chunk.id)}
                                    onCheckedChange={() => toggleSelection(chunk.id)}
                                    onClick={(e: React.MouseEvent) => e.stopPropagation()}
                                />
                                <button
                                    onClick={e => {
                                        e.preventDefault();
                                        e.stopPropagation();
                                        togglePin(chunk.id);
                                    }}
                                    className="text-muted-foreground hover:text-foreground"
                                >
                                    <Pin className={`size-3 ${isPinned(chunk.id) ? "fill-current" : ""}`} />
                                </button>
                                <Link
                                    to="/chunks/$chunkId"
                                    params={{ chunkId: chunk.id }}
                                    className="flex flex-1 items-center justify-between gap-4"
                                >
                                    <div className="min-w-0">
                                        <p className="truncate text-sm font-medium">{chunk.title}</p>
                                        <div className="mt-1 flex items-center gap-2">
                                            <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                                {chunk.type}
                                            </Badge>
                                            {([] as string[]).map(tag => (
                                                <Badge key={tag} variant="outline" size="sm" className="text-[10px]">
                                                    {tag}
                                                </Badge>
                                            ))}
                                        </div>
                                    </div>
                                    <div className="flex shrink-0 items-center gap-3">
                                        {(() => {
                                            const size = getChunkSize(chunk.content);
                                            return size.level !== "good" ? (
                                                <span className="flex items-center gap-1 text-xs" style={{ color: size.color }}>
                                                    <FileText className="size-3" />
                                                    {size.lines}L
                                                </span>
                                            ) : null;
                                        })()}
                                        <span className="text-muted-foreground flex items-center gap-1 text-xs">
                                            <Clock className="size-3" />
                                            {new Date(chunk.updatedAt).toLocaleDateString()}
                                        </span>
                                    </div>
                                </Link>
                            </CardPanel>
                        </div>
                    ))}
                </Card>
            )}

            {/* Pagination */}
            {view !== "kanban" && totalPages > 1 && (
                <div className="mt-4 flex items-center justify-center gap-2">
                    <Button variant="outline" size="sm" disabled={page <= 1} onClick={() => updateSearch({ page: page - 1 })}>
                        Previous
                    </Button>
                    <span className="text-muted-foreground text-sm">
                        Page {page} of {totalPages}
                    </span>
                    <Button variant="outline" size="sm" disabled={page >= totalPages} onClick={() => updateSearch({ page: page + 1 })}>
                        Next
                    </Button>
                </div>
            )}

            {selectedIds.size > 0 && (
                <div className="bg-background fixed bottom-6 left-1/2 z-50 flex -translate-x-1/2 items-center gap-3 rounded-lg border px-4 py-2.5 shadow-lg">
                    <span className="text-sm font-medium">{selectedIds.size} selected</span>
                    <Button variant="destructive" size="sm" onClick={handleBulkDelete} disabled={bulkDeleteMutation.isPending}>
                        <Trash2 className="size-3.5" />
                        Delete
                    </Button>
                    {collections.length > 0 && (
                        <select
                            onChange={e => {
                                if (e.target.value) {
                                    addToCollection(e.target.value, [...selectedIds]);
                                    setSelectedIds(new Set());
                                }
                            }}
                            className="bg-background rounded-md border px-2 py-1 text-xs"
                            defaultValue=""
                        >
                            <option value="" disabled>
                                Add to...
                            </option>
                            {collections.map(c => (
                                <option key={c.id} value={c.id}>
                                    {c.name}
                                </option>
                            ))}
                        </select>
                    )}
                    <Button variant="ghost" size="sm" onClick={() => setSelectedIds(new Set())}>
                        Cancel
                    </Button>
                </div>
            )}
        </div>
    );
}
