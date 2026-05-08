import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { Archive, Plus } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { ConfirmDialog } from "@/components/confirm-dialog";
import { PromptDialog } from "@/components/prompt-dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { ChunksResults } from "@/features/chunks/chunks-results";
import { ChunksToolbar } from "@/features/chunks/chunks-toolbar";
import { useChunksData } from "@/features/chunks/use-chunks-data";
import { useBulkChunkOperations } from "@/features/chunks/use-bulk-chunk-operations";
import { useChunkFilters } from "@/features/chunks/use-chunk-filters";
import { useSavedFilters } from "@/features/chunks/use-saved-filters";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { ImportDocsDialog } from "@/features/import/import-dialog";
import { ShortcutHint } from "@/features/nav/shortcut-hint";
import { getUser } from "@/functions/get-user";

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
        subGroup?: string;
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
        subGroup: (search.subGroup as string) || undefined,
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
    const navTo = useNavigate();
    const {
        type, q, sort, tags, size, after, enrichment, minConnections,
        group, subGroup, view, origin, reviewStatus, allCodebases,
        activeTags, activeFilterCount, hasActiveFilters, isFederated,
        updateSearch, clearAllFilters, toggleTag,
    } = useChunkFilters();
    const {
        selectedIds, setSelectedIds,
        bulkUpdateMutation, singleDeleteMutation, reviewMutation,
        handleSelectionClick, toggleAll,
    } = useBulkChunkOperations();
    const [searchInput, setSearchInput] = useState(q ?? "");
    const handleClearAllFilters = () => {
        clearAllFilters();
        setSearchInput("");
    };

    const { codebaseId } = useActiveCodebase();
    const { saveFilter } = useSavedFilters();

    const {
        activeQuery,
        tagsQuery,
        allChunks,
        processedChunks,
        editingChunkId,
        editTitle,
        setEditTitle,
        startEditing,
        commitEdit,
        cancelEdit,
        handleChunkHover,
        togglePin,
        isPinned,
    } = useChunksData({
        type,
        q,
        sort,
        tags,
        size,
        after,
        enrichment,
        minConnections,
        codebaseId,
        origin,
        reviewStatus,
        isFederated,
    });

    const chunks = allChunks;
    const total = activeQuery.data?.pages[0]?.total ?? 0;

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

    useEffect(() => {
        function handleKey(e: KeyboardEvent) {
            const tag = document.activeElement?.tagName;
            if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
            if (e.metaKey || e.ctrlKey || e.altKey) return;

            if (!/^[1-9]$/.test(e.key)) return;

            const n = Number(e.key) - 1;
            const chunksArray = processedChunks ?? [];
            if (n >= chunksArray.length) return;

            e.preventDefault();
            const chunk = chunksArray[n];
            if (chunk) {
                void navTo({ to: "/chunks/$chunkId", params: { chunkId: chunk.id } });
            }
        }
        window.addEventListener("keydown", handleKey);
        return () => window.removeEventListener("keydown", handleKey);
    }, [processedChunks, navTo]);

    const [confirmAction, setConfirmAction] = useState<{ title: string; description: string; action: () => void } | null>(null);
    const [showSaveFilter, setShowSaveFilter] = useState(false);

    const handleDeleteChunk = useCallback((id: string, _title: string) => {
        singleDeleteMutation.mutate(id);
    }, [singleDeleteMutation]);

    return (
        <div className="container mx-auto max-w-5xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <div className="flex items-center gap-3">
                    {chunks.length > 0 && (
                        <Checkbox
                            checked={selectedIds.size === chunks.length && chunks.length > 0}
                            indeterminate={selectedIds.size > 0 && selectedIds.size < chunks.length}
                            onCheckedChange={() => toggleAll(chunks.map(c => c.id))}
                        />
                    )}
                    <div>
                        <h1 className="text-2xl font-bold tracking-tight">Chunks</h1>
                        <p className="text-muted-foreground mt-1 text-xs">
                            <kbd className="bg-muted rounded px-1 py-0.5 font-mono text-[10px]">j</kbd>/
                            <kbd className="bg-muted rounded px-1 py-0.5 font-mono text-[10px]">k</kbd> navigate
                            <kbd className="bg-muted ml-2 rounded px-1 py-0.5 font-mono text-[10px]">Enter</kbd> open
                            <kbd className="bg-muted ml-2 rounded px-1 py-0.5 font-mono text-[10px]">n</kbd> new
                            <kbd className="bg-muted ml-2 rounded px-1 py-0.5 font-mono text-[10px]">Shift</kbd>+click range select
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

            <ChunksToolbar
                searchInput={searchInput}
                onSearchInputChange={setSearchInput}
                type={type}
                q={q}
                sort={sort}
                tags={tags}
                size={size}
                after={after}
                enrichment={enrichment}
                minConnections={minConnections}
                group={group}
                subGroup={subGroup}
                view={view}
                origin={origin}
                reviewStatus={reviewStatus}
                allCodebases={allCodebases}
                activeTags={activeTags}
                activeFilterCount={activeFilterCount}
                hasActiveFilters={hasActiveFilters}
                isFederated={isFederated}
                total={total}
                availableTags={tagsQuery.data ?? []}
                codebaseId={codebaseId}
                onUpdateSearch={updateSearch}
                onToggleTag={toggleTag}
                onClearAllFilters={handleClearAllFilters}
                onShowSaveFilter={() => setShowSaveFilter(true)}
                searchInputRef={searchInputRef}
            />

            <div className="mb-3">
                <ShortcutHint />
            </div>

            <ChunksResults
                view={view}
                group={group}
                subGroup={subGroup}
                sort={sort}
                type={type}
                q={q}
                tags={tags}
                origin={origin}
                reviewStatus={reviewStatus}
                codebaseId={codebaseId}
                isFederated={isFederated}
                isLoading={activeQuery.isLoading}
                hasNextPage={!!activeQuery.hasNextPage}
                isFetchingNextPage={activeQuery.isFetchingNextPage}
                allChunks={allChunks}
                processedChunks={processedChunks}
                total={total}
                selectedIds={selectedIds}
                setSelectedIds={setSelectedIds}
                selectedIndex={selectedIndex}
                editingChunkId={editingChunkId}
                editTitle={editTitle}
                onStartEditing={startEditing}
                onCommitEdit={commitEdit}
                onCancelEdit={cancelEdit}
                onEditTitleChange={setEditTitle}
                onHover={handleChunkHover}
                onTogglePin={togglePin}
                isPinned={isPinned}
                onSelectionClick={handleSelectionClick}
                onDeleteChunk={handleDeleteChunk}
                onReviewCycle={(id, next) => reviewMutation.mutate({ id, status: next })}
                onBulkDeleteChunks={(ids) => {
                    setConfirmAction({
                        title: "Delete chunks",
                        description: `Delete ${ids.size} chunks permanently?`,
                        action: () => bulkUpdateMutation.mutate({ ids: [...ids], action: "delete" }),
                    });
                }}
                onBulkArchiveChunks={(ids) => {
                    setConfirmAction({
                        title: "Archive chunks",
                        description: `Archive ${ids.size} chunks?`,
                        action: () => bulkUpdateMutation.mutate({ ids: [...ids], action: "archive" }),
                    });
                }}
                bulkUpdateMutation={bulkUpdateMutation}
                setConfirmAction={setConfirmAction}
                onFetchNextPage={() => activeQuery.fetchNextPage()}
            />

            <p className="text-muted-foreground mt-4 text-center text-xs">
                Press <kbd className="bg-muted rounded px-1 py-0.5 font-mono text-[10px]">?</kbd> for keyboard shortcuts
            </p>

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
