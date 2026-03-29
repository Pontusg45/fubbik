import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import {
    Archive,
    Bot,
    ChevronRight,
    Clock,
    Columns3,
    FileText,
    Filter,
    FolderPlus,
    Globe,
    List,
    Pin,
    Plus,
    Search,
    Server,
    X
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { PromptDialog } from "@/components/prompt-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { Tooltip, TooltipTrigger, TooltipPopup } from "@/components/ui/tooltip";
import { ChunkFiltersPopover } from "@/features/chunks/chunk-filters-popover";
import { getChunkSize } from "@/features/chunks/chunk-size";
import { ChunkBulkActionBar } from "@/features/chunks/chunk-bulk-action-bar";
import { ChunkRowActions } from "@/features/chunks/chunk-row-actions";
import { KanbanView } from "@/features/chunks/kanban-view";
import { useCollections } from "@/features/chunks/use-collections";
import { usePinnedChunks } from "@/features/chunks/use-pinned-chunks";
import { useSavedFilters } from "@/features/chunks/use-saved-filters";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { ImportDocsDialog } from "@/features/import/import-dialog";
import { ShortcutHint } from "@/features/nav/shortcut-hint";
import { useIntersectionObserver } from "@/hooks/use-intersection-observer";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/chunks/")({
    component: ChunksList,
    validateSearch: (search: Record<string, unknown>): {
        type?: string;
        q?: string;
        sort?: string;
        tags?: string;
        size?: string;
        after?: string;
        enrichment?: string;
        minConnections?: string;
        group?: string;
        collection?: string;
        view?: string;
        origin?: string;
        reviewStatus?: string;
        allCodebases?: string;
    } => ({
        type: (search.type as string) || undefined,
        q: (search.q as string) || undefined,
        sort: (search.sort as string) || undefined,
        tags: (search.tags as string) || undefined,
        size: (search.size as string) || undefined,
        after: (search.after as string) || undefined,
        enrichment: (search.enrichment as string) || undefined,
        minConnections: (search.minConnections as string) || undefined,
        group: (search.group as string) || undefined,
        collection: (search.collection as string) || undefined,
        view: (search.view as string) || undefined,
        origin: (search.origin as string) || undefined,
        reviewStatus: (search.reviewStatus as string) || undefined,
        allCodebases: (search.allCodebases as string) || undefined
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
    const { type, q, sort, tags, size, after, enrichment, minConnections, group, collection, view, origin, reviewStatus, allCodebases } = Route.useSearch();
    const queryClient = useQueryClient();
    const [searchInput, setSearchInput] = useState(q ?? "");
    const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
    const { filters: savedFilters, saveFilter, deleteFilter } = useSavedFilters();
    const { codebaseId } = useActiveCodebase();
    const limit = 20;

    const activeFilterCount = [tags, size, after, enrichment, minConnections, origin, reviewStatus].filter(Boolean).length;
    const hasActiveFilters = !!(type || q || sort || tags || size || after || enrichment || minConnections || origin || reviewStatus);

    const isFederated = allCodebases === "true";

    const chunksQuery = useInfiniteQuery({
        queryKey: ["chunks-list", type, q, sort, tags, after, enrichment, minConnections, codebaseId, origin, reviewStatus],
        queryFn: async ({ pageParam = 1 }) => {
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
                            offset: String((pageParam - 1) * limit),
                            ...(codebaseId === "global" ? { global: "true" } : codebaseId ? { codebaseId } : {}),
                            origin: origin as "human" | "ai" | undefined,
                            reviewStatus: reviewStatus as "draft" | "reviewed" | "approved" | undefined
                        }
                    })
                );
            } catch {
                return null;
            }
        },
        enabled: !isFederated,
        initialPageParam: 1,
        getNextPageParam: (lastPage, allPages) => {
            if (!lastPage) return undefined;
            const loaded = allPages.reduce((sum, p) => sum + (p?.chunks?.length ?? 0), 0);
            return loaded < lastPage.total ? allPages.length + 1 : undefined;
        }
    });

    const federatedQuery = useInfiniteQuery({
        queryKey: ["chunks-federated", type, q, sort, tags, origin, reviewStatus],
        queryFn: async ({ pageParam = 1 }) => {
            try {
                const res = await api.api.chunks.search.federated.get({
                    query: {
                        type,
                        search: q,
                        sort: sort as "newest" | "oldest" | "alpha" | "updated" | undefined,
                        tags,
                        limit: String(limit),
                        offset: String((pageParam - 1) * limit),
                    }
                });
                return unwrapEden(res);
            } catch {
                return null;
            }
        },
        enabled: isFederated,
        initialPageParam: 1,
        getNextPageParam: (lastPage, allPages) => {
            if (!lastPage) return undefined;
            const loaded = allPages.reduce((sum, p) => sum + (p?.chunks?.length ?? 0), 0);
            return loaded < lastPage.total ? allPages.length + 1 : undefined;
        }
    });

    const activeQuery = isFederated ? federatedQuery : chunksQuery;

    const tagsQuery = useQuery({
        queryKey: ["tags"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.tags.get());
            } catch {
                return [];
            }
        },
        staleTime: 60_000
    });

    const activeTags = tags ? tags.split(",") : [];

    function toggleTag(tag: string) {
        const current = activeTags;
        const next = current.includes(tag) ? current.filter(t => t !== tag) : [...current, tag];
        updateSearch({ tags: next.length > 0 ? next.join(",") : undefined });
    }

    const allChunks = activeQuery.data?.pages.flatMap(p => p?.chunks ?? []) ?? [];
    const chunks = allChunks;
    const { pinnedIds, togglePin, isPinned } = usePinnedChunks();
    const { collections, createCollection, deleteCollection: deleteCol } = useCollections();

    const processedChunks = useMemo(() => {
        const source = allChunks;
        const filtered = size ? source.filter(c => getChunkSize(c.content).level === size) : source;
        const pinnedSet = new Set(pinnedIds);
        return [...filtered].sort((a, b) => {
            const aPinned = pinnedSet.has(a.id) ? 0 : 1;
            const bPinned = pinnedSet.has(b.id) ? 0 : 1;
            return aPinned - bPinned;
        });
        // eslint-disable-next-line react-hooks/exhaustive-deps -- allChunks is stable per fetch
    }, [activeQuery.data, size, pinnedIds]);

    const collectionFilteredChunks = processedChunks;

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
    const searchInputRef = useRef<HTMLInputElement>(null);
    const [selectedIndex, setSelectedIndex] = useState(-1);

    useEffect(() => {
        setSelectedIndex(-1);
    }, [activeQuery.dataUpdatedAt]);

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
                case "/":
                    e.preventDefault();
                    searchInputRef.current?.focus();
                    break;
            }
        }
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [selectedIndex, navTo]);

    const total = activeQuery.data?.pages[0]?.total ?? 0;

    const fetchNextPageCb = useCallback(() => {
        activeQuery.fetchNextPage();
    }, [activeQuery]);
    const loadMoreRef = useIntersectionObserver(
        fetchNextPageCb,
        !!activeQuery.hasNextPage && !activeQuery.isFetchingNextPage
    );

    function updateSearch(
        params: Partial<{
            type: string;
            q: string;
            sort: string;
            tags: string;
            size: string;
            after: string;
            enrichment: string;
            minConnections: string;
            group: string;
            collection: string;
            view: string;
            origin: string;
            reviewStatus: string;
            allCodebases: string;
        }>
    ) {
        navigate({
            search: {
                type: params.type !== undefined ? params.type : type,
                q: params.q !== undefined ? params.q : q,
                sort: params.sort !== undefined ? params.sort : sort,
                tags: params.tags !== undefined ? params.tags : tags,
                size: params.size !== undefined ? params.size : size,
                after: params.after !== undefined ? params.after : after,
                enrichment: params.enrichment !== undefined ? params.enrichment : enrichment,
                minConnections: params.minConnections !== undefined ? params.minConnections : minConnections,
                group: params.group !== undefined ? params.group : group,
                collection: params.collection !== undefined ? params.collection : collection,
                view: params.view !== undefined ? params.view : view,
                origin: params.origin !== undefined ? params.origin : origin,
                reviewStatus: params.reviewStatus !== undefined ? params.reviewStatus : reviewStatus,
                allCodebases: params.allCodebases !== undefined ? params.allCodebases : allCodebases
            }
        });
    }

    function clearAllFilters() {
        navigate({
            search: {
                type: undefined,
                q: undefined,
                sort: undefined,
                tags: undefined,
                size: undefined,
                after: undefined,
                enrichment: undefined,
                minConnections: undefined,
                group,
                collection,
                view,
                origin: undefined,
                reviewStatus: undefined,
                allCodebases: undefined
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



    const bulkUpdateMutation = useMutation({
        mutationFn: async (body: { ids: string[]; action: string; value?: string | null }) => {
            const { error } = await (api.api.chunks as any)["bulk-update"].post(body);
            if (error) throw new Error("Bulk update failed");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            setSelectedIds(new Set());
        }
    });

    const singleDeleteMutation = useMutation({
        mutationFn: async (id: string) => {
            const { error } = await api.api.chunks({ id }).delete();
            if (error) throw new Error("Failed to delete chunk");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            toast.success("Chunk deleted");
        },
        onError: () => {
            toast.error("Failed to delete chunk");
        }
    });

    const reviewMutation = useMutation({
        mutationFn: async ({ id, status }: { id: string; status: string }) => {
            const { error } = await api.api.chunks({ id }).patch({ reviewStatus: status as any });
            if (error) throw new Error("Failed to update review status");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
        },
        onError: () => {
            toast.error("Failed to update review status");
        }
    });

    const [confirmAction, setConfirmAction] = useState<{ title: string; description: string; action: () => void } | null>(null);
    const [showSaveFilter, setShowSaveFilter] = useState(false);


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
                <div className="flex items-center gap-2">
                    <Link to="/chunks/archived" className="text-muted-foreground flex items-center gap-1 text-xs hover:underline">
                        <Archive className="size-3.5" />
                        View archived
                    </Link>
                    <ImportDocsDialog />
                    <Button render={<Link to="/chunks/new" />}>
                        <Plus className="size-4" />
                        New Chunk
                    </Button>
                </div>
            </div>

            {/* Search bar + controls */}
            <div className="mb-4 flex flex-wrap items-center gap-2">
                <div className="relative flex-1">
                    <Search className="text-muted-foreground absolute top-1/2 left-3 size-4 -translate-y-1/2" />
                    <input
                        ref={searchInputRef}
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

                {/* All codebases toggle */}
                <Button
                    variant={isFederated ? "default" : "outline"}
                    size="sm"
                    onClick={() => updateSearch({ allCodebases: isFederated ? undefined : "true" })}
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
                    availableTags={tagsQuery.data ?? []}
                    codebaseId={codebaseId}
                    onUpdateSearch={updateSearch}
                    onToggleTag={toggleTag}
                    onClearAllFilters={clearAllFilters}
                    onShowSaveFilter={() => setShowSaveFilter(true)}
                    onCreateCollection={createCollection}
                />

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
                                                navigate({
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
                    {origin && (
                        <Badge variant="secondary" size="sm" className="gap-1">
                            Origin: {origin}
                            <button onClick={() => updateSearch({ origin: undefined })}>
                                <X className="size-2.5" />
                            </button>
                        </Badge>
                    )}
                    {reviewStatus && (
                        <Badge variant="secondary" size="sm" className="gap-1">
                            Review: {reviewStatus}
                            <button onClick={() => updateSearch({ reviewStatus: undefined })}>
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
                            onClick={() => navigate({ search: { ...f.params } })}
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

            <div className="mb-3">
                <ShortcutHint />
            </div>

            {/* Results */}
            {view === "kanban" ? (
                <KanbanView
                    chunks={collectionFilteredChunks}
                    onBulkDelete={(ids) => {
                        setConfirmAction({
                            title: "Delete chunks",
                            description: `Delete ${ids.size} chunks permanently?`,
                            action: () => bulkUpdateMutation.mutate({ ids: [...ids], action: "delete" }),
                        });
                    }}
                    onBulkArchive={(ids) => {
                        setConfirmAction({
                            title: "Archive chunks",
                            description: `Archive ${ids.size} chunks?`,
                            action: () => bulkUpdateMutation.mutate({ ids: [...ids], action: "archive" }),
                        });
                    }}
                />
            ) : activeQuery.isLoading ? (
                <SkeletonList count={10} />
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
                                                        <Tooltip>
                                                            <TooltipTrigger render={<p className="truncate text-sm font-medium" />}>
                                                                {chunk.title}
                                                            </TooltipTrigger>
                                                            {chunk.content && (
                                                                <TooltipPopup className="max-w-xs p-2">
                                                                    <p className="text-xs text-muted-foreground">{chunk.content.slice(0, 150)}{chunk.content.length > 150 ? "..." : ""}</p>
                                                                </TooltipPopup>
                                                            )}
                                                        </Tooltip>
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
                                                <div onClick={e => e.stopPropagation()}>
                                                    <ChunkRowActions
                                                        chunkId={chunk.id}
                                                        isPinned={isPinned(chunk.id)}
                                                        onTogglePin={() => togglePin(chunk.id)}
                                                        onDelete={() =>
                                                            setConfirmAction({
                                                                title: "Delete chunk",
                                                                description: `Delete "${chunk.title}" permanently?`,
                                                                action: () => singleDeleteMutation.mutate(chunk.id)
                                                            })
                                                        }
                                                    />
                                                </div>
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
                                        <Tooltip>
                                            <TooltipTrigger render={<p className="truncate text-sm font-medium" />}>
                                                {chunk.title}
                                            </TooltipTrigger>
                                            {chunk.content && (
                                                <TooltipPopup className="max-w-xs p-2">
                                                    <p className="text-xs text-muted-foreground">{chunk.content.slice(0, 150)}{chunk.content.length > 150 ? "..." : ""}</p>
                                                </TooltipPopup>
                                            )}
                                        </Tooltip>
                                        <div className="mt-1 flex items-center gap-2">
                                            <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                                {chunk.type}
                                            </Badge>
                                            {isFederated && !!(chunk as Record<string, unknown>).codebaseName && (
                                                <Badge variant="outline" size="sm" className="border-blue-500/30 bg-blue-500/10 text-[10px] text-blue-600">
                                                    <Server className="mr-0.5 size-2.5" />
                                                    {String((chunk as Record<string, unknown>).codebaseName)}
                                                </Badge>
                                            )}
                                            {(chunk as Record<string, unknown>).origin === "ai" && (
                                                <>
                                                    <Badge
                                                        variant="outline"
                                                        size="sm"
                                                        className={
                                                            (chunk as Record<string, unknown>).reviewStatus === "draft"
                                                                ? "border-yellow-500/30 bg-yellow-500/10 text-[10px] text-yellow-600"
                                                                : (chunk as Record<string, unknown>).reviewStatus === "reviewed"
                                                                  ? "border-blue-500/30 bg-blue-500/10 text-[10px] text-blue-600"
                                                                  : "border-green-500/30 bg-green-500/10 text-[10px] text-green-600"
                                                        }
                                                    >
                                                        <Bot className="mr-0.5 size-2.5" />
                                                        AI
                                                    </Badge>
                                                    <button
                                                        onClick={(e) => {
                                                            e.preventDefault();
                                                            e.stopPropagation();
                                                            const next = { draft: "reviewed", reviewed: "approved", approved: "draft" }[(chunk as Record<string, unknown>).reviewStatus as string] ?? "reviewed";
                                                            reviewMutation.mutate({ id: chunk.id, status: next });
                                                        }}
                                                        className="size-2.5 rounded-full shrink-0"
                                                        style={{
                                                            backgroundColor:
                                                                (chunk as Record<string, unknown>).reviewStatus === "approved" ? "#22c55e"
                                                                : (chunk as Record<string, unknown>).reviewStatus === "reviewed" ? "#3b82f6"
                                                                : "#f59e0b"
                                                        }}
                                                        title={`Review: ${(chunk as Record<string, unknown>).reviewStatus ?? "draft"} (click to change)`}
                                                    />
                                                </>
                                            )}
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
                                <div onClick={e => e.stopPropagation()}>
                                    <ChunkRowActions
                                        chunkId={chunk.id}
                                        isPinned={isPinned(chunk.id)}
                                        onTogglePin={() => togglePin(chunk.id)}
                                        onDelete={() =>
                                            setConfirmAction({
                                                title: "Delete chunk",
                                                description: `Delete "${chunk.title}" permanently?`,
                                                action: () => singleDeleteMutation.mutate(chunk.id)
                                            })
                                        }
                                    />
                                </div>
                            </CardPanel>
                        </div>
                    ))}
                </Card>
            )}

            {/* Load more trigger */}
            {view !== "kanban" && activeQuery.hasNextPage && (
                <div ref={loadMoreRef} className="mt-4 flex justify-center">
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => activeQuery.fetchNextPage()}
                        disabled={activeQuery.isFetchingNextPage}
                    >
                        {activeQuery.isFetchingNextPage ? "Loading..." : "Load more"}
                    </Button>
                </div>
            )}
            {view !== "kanban" && total > 0 && (
                <p className="text-muted-foreground mt-2 text-center text-xs">
                    Showing {allChunks.length} of {total} chunks
                </p>
            )}

            <ChunkBulkActionBar
                selectedIds={selectedIds}
                setSelectedIds={setSelectedIds}
                bulkUpdateMutation={bulkUpdateMutation}
                setConfirmAction={setConfirmAction}
            />

            <ConfirmDialog
                open={confirmAction !== null}
                onOpenChange={(open) => { if (!open) setConfirmAction(null); }}
                title={confirmAction?.title ?? ""}
                description={confirmAction?.description}
                confirmLabel="Confirm"
                confirmVariant="destructive"
                onConfirm={() => {
                    confirmAction?.action();
                    setConfirmAction(null);
                }}
                loading={bulkUpdateMutation.isPending}
            />

            <PromptDialog
                open={showSaveFilter}
                onOpenChange={setShowSaveFilter}
                title="Save filter preset"
                description="Give this filter combination a name so you can quickly apply it later."
                placeholder="Filter name"
                submitLabel="Save"
                onSubmit={(name) => {
                    saveFilter(name, { type, q, sort, tags, size, after, enrichment, minConnections });
                    setShowSaveFilter(false);
                }}
            />
        </div>
    );
}
