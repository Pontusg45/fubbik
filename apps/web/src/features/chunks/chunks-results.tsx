import { Link } from "@tanstack/react-router";
import { FileText, Plus } from "lucide-react";
import { useCallback, useMemo } from "react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { PageEmpty } from "@/components/ui/page";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { ChunkBulkActionBar } from "@/features/chunks/chunk-bulk-action-bar";
import { ChunkCardGrid } from "@/features/chunks/chunk-card-grid";
import { ChunkRow, type ChunkRowChunk } from "@/features/chunks/chunk-row";
import { KanbanView } from "@/features/chunks/kanban-view";
import { LazyGroupList } from "@/features/chunks/lazy-group-list";
import { useIntersectionObserver } from "@/hooks/use-intersection-observer";

type ConfirmAction = { title: string; description: string; action: () => void };

interface ChunksResultsProps {
    view?: string;
    group?: string;
    subGroup?: string;
    sort?: string;
    type?: string;
    q?: string;
    tags?: string;
    origin?: string;
    reviewStatus?: string;
    codebaseId?: string | null;
    isFederated: boolean;
    isLoading: boolean;
    hasNextPage: boolean;
    isFetchingNextPage: boolean;
    allChunks: ChunkRowChunk[];
    processedChunks: ChunkRowChunk[];
    total: number;
    selectedIds: Set<string>;
    setSelectedIds: (ids: Set<string>) => void;
    selectedIndex: number;
    editingChunkId: string | null;
    editTitle: string;
    onStartEditing: (id: string, title: string) => void;
    onCommitEdit: () => void;
    onCancelEdit: () => void;
    onEditTitleChange: (v: string) => void;
    onHover: (id: string) => void;
    onTogglePin: (id: string) => void;
    isPinned: (id: string) => boolean;
    onSelectionClick: (id: string, index: number, allIds: string[], e: React.MouseEvent) => void;
    onDeleteChunk: (id: string, title: string) => void;
    onReviewCycle: (id: string, next: string) => void;
    onBulkDeleteChunks: (ids: Set<string>) => void;
    onBulkArchiveChunks: (ids: Set<string>) => void;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    bulkUpdateMutation: any;
    setConfirmAction: (action: ConfirmAction | null) => void;
    onFetchNextPage: () => void;
}

export function ChunksResults({
    view,
    group,
    subGroup,
    sort,
    type,
    q,
    tags,
    origin,
    reviewStatus,
    codebaseId,
    isFederated,
    isLoading,
    hasNextPage,
    isFetchingNextPage,
    allChunks,
    processedChunks,
    total,
    selectedIds,
    setSelectedIds,
    selectedIndex,
    editingChunkId,
    editTitle,
    onStartEditing,
    onCommitEdit,
    onCancelEdit,
    onEditTitleChange,
    onHover,
    onTogglePin,
    isPinned,
    onSelectionClick,
    onDeleteChunk,
    onReviewCycle,
    onBulkDeleteChunks,
    onBulkArchiveChunks,
    bulkUpdateMutation,
    setConfirmAction,
    onFetchNextPage,
}: ChunksResultsProps) {
    const hasGrouping = !!group;
    const selectedTagTypeId = group?.startsWith("tagtype:") ? group.slice("tagtype:".length) : null;

    const allChunkIds = useMemo(() => processedChunks.map(c => c.id), [processedChunks]);

    const fetchNextPageCb = useCallback(() => {
        onFetchNextPage();
    }, [onFetchNextPage]);

    const loadMoreRef = useIntersectionObserver(fetchNextPageCb, hasNextPage && !isFetchingNextPage);

    return (
        <>
            {/* Results */}
            {view === "kanban" ? (
                <KanbanView
                    chunks={processedChunks}
                    onBulkDelete={(ids) => {
                        onBulkDeleteChunks(ids);
                    }}
                    onBulkArchive={(ids) => {
                        onBulkArchiveChunks(ids);
                    }}
                />
            ) : isLoading ? (
                <SkeletonList count={10} />
            ) : view === "grid" ? (
                <ChunkCardGrid chunks={processedChunks} />
            ) : processedChunks.length === 0 && !hasGrouping ? (
                <PageEmpty
                    icon={FileText}
                    title="No chunks yet"
                    description="Create your first chunk to start building your knowledge base"
                    action={
                        <Button size="sm" render={<Link to="/chunks/new" />}>
                            <Plus className="mr-1 size-4" />
                            New Chunk
                        </Button>
                    }
                />
            ) : hasGrouping ? (
                <LazyGroupList
                    groupBy={group!}
                    tagTypeId={selectedTagTypeId}
                    subGroupBy={subGroup}
                    codebaseId={codebaseId}
                    sort={sort}
                    filters={{
                        type,
                        tags,
                        origin,
                        reviewStatus,
                        search: q,
                    }}
                    selectedIds={selectedIds}
                    onSelectionClick={onSelectionClick}
                    onDelete={(id, title) =>
                        setConfirmAction({
                            title: "Delete chunk",
                            description: `Delete "${title}" permanently?`,
                            action: () => onDeleteChunk(id, title),
                        })
                    }
                    onReviewCycle={(id, next) => onReviewCycle(id, next)}
                />
            ) : (
                <Card>
                    {processedChunks.map((chunk, i) => (
                        <ChunkRow
                            key={chunk.id}
                            chunk={chunk}
                            index={i}
                            allChunkIds={allChunkIds}
                            isSelected={selectedIds.has(chunk.id)}
                            isPinned={isPinned(chunk.id)}
                            isKeyboardSelected={selectedIndex === i}
                            showExtendedBadges
                            isFederated={isFederated}
                            showSeparator={i > 0}
                            editingChunkId={editingChunkId}
                            editTitle={editTitle}
                            onStartEditing={onStartEditing}
                            onCommitEdit={onCommitEdit}
                            onCancelEdit={onCancelEdit}
                            onEditTitleChange={onEditTitleChange}
                            onHover={onHover}
                            onTogglePin={onTogglePin}
                            onSelectionClick={onSelectionClick}
                            onDelete={(id, title) =>
                                setConfirmAction({
                                    title: "Delete chunk",
                                    description: `Delete "${title}" permanently?`,
                                    action: () => onDeleteChunk(id, title),
                                })
                            }
                            onReviewCycle={(id, next) => onReviewCycle(id, next)}
                        />
                    ))}
                </Card>
            )}

            {/* Load more trigger */}
            {view !== "kanban" && hasNextPage && (
                <div ref={loadMoreRef} className="mt-4 flex justify-center">
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={onFetchNextPage}
                        disabled={isFetchingNextPage}
                    >
                        {isFetchingNextPage ? "Loading..." : "Load more"}
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
        </>
    );
}
