import { useCallback, useRef, useState } from "react";
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ChevronRight, Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { ChunkRow, type ChunkRowChunk } from "@/features/chunks/chunk-row";
import { usePinnedChunks } from "@/features/chunks/use-pinned-chunks";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface LazyGroupListFilters {
    type?: string;
    tags?: string;
    tagMode?: "any" | "all";
    origin?: string;
    reviewStatus?: string;
    search?: string;
}

interface LazyGroupListProps {
    groupBy: string;
    tagTypeId?: string | null;
    codebaseId?: string | null;
    workspaceId?: string;
    sort?: string;
    filters: LazyGroupListFilters;

    // Bulk selection support
    selectedIds: Set<string>;
    onSelectionClick: (id: string, index: number, allIds: string[], e: React.MouseEvent) => void;

    // Deletion
    onDelete: (id: string, title: string) => void;

    // Review cycle
    onReviewCycle?: (id: string, currentStatus: string) => void;
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function LazyGroupList({
    groupBy,
    tagTypeId,
    codebaseId,
    workspaceId,
    sort,
    filters,
    selectedIds,
    onSelectionClick,
    onDelete,
    onReviewCycle,
}: LazyGroupListProps) {
    const [expanded, setExpanded] = useState<Set<string>>(new Set());

    const queryParam = buildGroupByParam(groupBy, tagTypeId);

    const groupsQuery = useQuery({
        queryKey: ["chunks-grouped", queryParam, tagTypeId, codebaseId, workspaceId, filters],
        queryFn: async () => {
            return unwrapEden(
                await api.api.chunks.grouped.get({
                    query: {
                        groupBy: queryParam,
                        ...(tagTypeId ? { tagTypeId } : {}),
                        ...(codebaseId && codebaseId !== "global"
                            ? { codebaseId }
                            : codebaseId === "global"
                              ? { global: "true" }
                              : {}),
                        ...(workspaceId ? { workspaceId } : {}),
                        ...(filters.type ? { type: filters.type } : {}),
                        ...(filters.tags ? { tags: filters.tags } : {}),
                        ...(filters.tagMode ? { tagMode: filters.tagMode } : {}),
                        ...(filters.origin
                            ? { origin: filters.origin as "human" | "ai" }
                            : {}),
                        ...(filters.reviewStatus
                            ? { reviewStatus: filters.reviewStatus as "draft" | "reviewed" | "approved" }
                            : {}),
                        ...(filters.search ? { search: filters.search } : {}),
                    },
                })
            );
        },
        staleTime: 30_000,
    });

    function toggleExpanded(name: string) {
        setExpanded(prev => {
            const next = new Set(prev);
            if (next.has(name)) next.delete(name);
            else next.add(name);
            return next;
        });
    }

    if (groupsQuery.isLoading) {
        return <SkeletonList count={5} />;
    }

    const groups = groupsQuery.data?.groups ?? [];

    if (groups.length === 0) {
        return (
            <p className="text-muted-foreground py-8 text-center text-sm">
                No groups found for this grouping.
            </p>
        );
    }

    return (
        <div className="space-y-4">
            {groups.map(g => (
                <div key={g.groupName}>
                    <button
                        onClick={() => toggleExpanded(g.groupName)}
                        className="mb-2 flex items-center gap-2"
                    >
                        <ChevronRight
                            className={`size-3 transition-transform ${expanded.has(g.groupName) && "rotate-90"}`}
                        />
                        <Badge variant="secondary">{g.groupName}</Badge>
                        <span className="text-muted-foreground text-xs">
                            ({g.count})
                        </span>
                    </button>
                    {expanded.has(g.groupName) && (
                        <GroupChunksList
                            groupName={g.groupName}
                            groupBy={queryParam}
                            tagTypeId={tagTypeId}
                            codebaseId={codebaseId}
                            workspaceId={workspaceId}
                            sort={sort}
                            filters={filters}
                            selectedIds={selectedIds}
                            onSelectionClick={onSelectionClick}
                            onDelete={onDelete}
                            onReviewCycle={onReviewCycle}
                        />
                    )}
                </div>
            ))}
        </div>
    );
}

// ---------------------------------------------------------------------------
// Per-group chunk list with infinite query + virtual scrolling
// ---------------------------------------------------------------------------

const PAGE_SIZE = 50;
const ESTIMATED_ROW_HEIGHT = 72;

interface GroupChunksListProps {
    groupName: string;
    groupBy: string;
    tagTypeId?: string | null;
    codebaseId?: string | null;
    workspaceId?: string;
    sort?: string;
    filters: LazyGroupListFilters;
    selectedIds: Set<string>;
    onSelectionClick: (id: string, index: number, allIds: string[], e: React.MouseEvent) => void;
    onDelete: (id: string, title: string) => void;
    onReviewCycle?: (id: string, currentStatus: string) => void;
}

function GroupChunksList({
    groupName,
    groupBy,
    tagTypeId,
    codebaseId,
    workspaceId,
    sort,
    filters,
    selectedIds,
    onSelectionClick,
    onDelete,
    onReviewCycle,
}: GroupChunksListProps) {
    const queryClient = useQueryClient();
    const { isPinned, togglePin } = usePinnedChunks();

    // Inline title editing
    const [editingChunkId, setEditingChunkId] = useState<string | null>(null);
    const [editTitle, setEditTitle] = useState("");
    const editMutation = useMutation({
        mutationFn: async ({ id, title }: { id: string; title: string }) =>
            unwrapEden(await api.api.chunks({ id }).patch({ title })),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-group", groupName] });
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
        },
    });

    const chunksQuery = useInfiniteQuery({
        queryKey: [
            "chunks-group",
            groupName,
            groupBy,
            tagTypeId,
            codebaseId,
            workspaceId,
            sort,
            filters,
        ],
        queryFn: async ({ pageParam = 0 }) => {
            return unwrapEden(
                await api.api.chunks.grouped({ groupName }).chunks.get({
                    query: {
                        groupBy,
                        ...(tagTypeId ? { tagTypeId } : {}),
                        ...(codebaseId && codebaseId !== "global"
                            ? { codebaseId }
                            : codebaseId === "global"
                              ? { global: "true" }
                              : {}),
                        ...(workspaceId ? { workspaceId } : {}),
                        ...(sort
                            ? { sort: sort as "newest" | "oldest" | "alpha" | "updated" }
                            : {}),
                        ...(filters.type ? { type: filters.type } : {}),
                        ...(filters.tags ? { tags: filters.tags } : {}),
                        ...(filters.tagMode ? { tagMode: filters.tagMode } : {}),
                        ...(filters.origin
                            ? { origin: filters.origin as "human" | "ai" }
                            : {}),
                        ...(filters.reviewStatus
                            ? { reviewStatus: filters.reviewStatus as "draft" | "reviewed" | "approved" }
                            : {}),
                        ...(filters.search ? { search: filters.search } : {}),
                        limit: String(PAGE_SIZE),
                        offset: String(pageParam),
                    },
                })
            );
        },
        initialPageParam: 0,
        getNextPageParam: (lastPage, allPages) => {
            if (!lastPage) return undefined;
            const loaded = allPages.reduce(
                (sum, p) => sum + (p?.chunks?.length ?? 0),
                0
            );
            return loaded < lastPage.total ? loaded : undefined;
        },
    });

    const allChunks: ChunkRowChunk[] = (
        chunksQuery.data?.pages.flatMap(p => p?.chunks ?? []) ?? []
    ) as ChunkRowChunk[];

    const allChunkIds = allChunks.map(c => c.id);

    // Virtualizer
    const parentRef = useRef<HTMLDivElement>(null);
    const virtualizer = useVirtualizer({
        count: allChunks.length,
        getScrollElement: () => parentRef.current,
        estimateSize: () => ESTIMATED_ROW_HEIGHT,
        overscan: 5,
    });

    // Fetch next page when scrolling near bottom
    const handleScroll = useCallback(() => {
        const el = parentRef.current;
        if (!el) return;
        const distFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
        if (distFromBottom < 200 && chunksQuery.hasNextPage && !chunksQuery.isFetchingNextPage) {
            chunksQuery.fetchNextPage();
        }
    }, [chunksQuery]);

    const handleChunkHover = (chunkId: string) => {
        queryClient.prefetchQuery({
            queryKey: ["chunk", chunkId],
            queryFn: async () => unwrapEden(await api.api.chunks({ id: chunkId }).get()),
            staleTime: 30_000,
        });
    };

    if (chunksQuery.isLoading) {
        return <SkeletonList count={3} />;
    }

    if (allChunks.length === 0) {
        return (
            <p className="text-muted-foreground py-4 text-center text-sm">
                No chunks in this group.
            </p>
        );
    }

    const virtualItems = virtualizer.getVirtualItems();

    return (
        <Card>
            <div
                ref={parentRef}
                onScroll={handleScroll}
                className="overflow-auto"
                style={{ maxHeight: 600 }}
            >
                <div
                    style={{
                        height: virtualizer.getTotalSize(),
                        width: "100%",
                        position: "relative",
                    }}
                >
                    {virtualItems.map(virtualRow => {
                        const chunk = allChunks[virtualRow.index]!;
                        return (
                            <div
                                key={chunk.id}
                                data-index={virtualRow.index}
                                ref={virtualizer.measureElement}
                                style={{
                                    position: "absolute",
                                    top: 0,
                                    left: 0,
                                    width: "100%",
                                    transform: `translateY(${virtualRow.start}px)`,
                                }}
                            >
                                <ChunkRow
                                    chunk={chunk}
                                    index={virtualRow.index}
                                    allChunkIds={allChunkIds}
                                    isSelected={selectedIds.has(chunk.id)}
                                    isPinned={isPinned(chunk.id)}
                                    showSeparator={virtualRow.index > 0}
                                    showExtendedBadges
                                    editingChunkId={editingChunkId}
                                    editTitle={editTitle}
                                    onStartEditing={(id, title) => {
                                        setEditingChunkId(id);
                                        setEditTitle(title);
                                    }}
                                    onCommitEdit={() => {
                                        if (editingChunkId && editTitle.trim()) {
                                            editMutation.mutate({
                                                id: editingChunkId,
                                                title: editTitle.trim(),
                                            });
                                        }
                                        setEditingChunkId(null);
                                    }}
                                    onCancelEdit={() => setEditingChunkId(null)}
                                    onEditTitleChange={setEditTitle}
                                    onHover={handleChunkHover}
                                    onTogglePin={togglePin}
                                    onSelectionClick={onSelectionClick}
                                    onDelete={onDelete}
                                    onReviewCycle={onReviewCycle}
                                />
                            </div>
                        );
                    })}
                </div>
            </div>
            {chunksQuery.isFetchingNextPage && (
                <div className="flex justify-center py-2">
                    <Loader2 className="text-muted-foreground size-4 animate-spin" />
                </div>
            )}
        </Card>
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Build the groupBy query parameter. For tag type groups the URL param is
 * `tagtype:<id>` but the API expects `groupBy=tagtype` + separate `tagTypeId`.
 * For other groups, just pass through.
 */
function buildGroupByParam(
    groupBy: string,
    tagTypeId?: string | null
): string {
    if (tagTypeId || groupBy.startsWith("tagtype:")) return "tagtype";
    return groupBy;
}
