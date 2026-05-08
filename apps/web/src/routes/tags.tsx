import { createFileRoute } from "@tanstack/react-router";
import {
    ArrowUpDown,
    Palette,
    Pencil,
    Plus,
    Search,
    Tag as TagIcon,
    Tags as TagsIcon,
    Trash2,
    X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from "@/components/ui/dialog";
import { PageContainer, PageEmpty, PageHeader, PageLoading } from "@/components/ui/page";
import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { getUser } from "@/functions/get-user";

import { TagPill } from "@/features/tags/tag-pill";
import { useTagsData } from "@/features/tags/use-tags-data";
import type { Tag, SortMode } from "@/features/tags/tag-types";

export const Route = createFileRoute("/tags")({
    component: TagsPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

function TagsPage() {
    const {
        tagsQuery,
        tagTypesQuery,
        tags,
        tagTypes,
        filteredTags,
        sortedGroups,
        unusedCount,
        search,
        setSearch,
        sortMode,
        setSortMode,
        unusedOnly,
        setUnusedOnly,
        renamingId,
        renameValue,
        setRenameValue,
        startRename,
        commitRename,
        cancelRename,
        showTagTypeForm,
        setShowTagTypeForm,
        editingTagType,
        ttName,
        setTtName,
        ttColor,
        setTtColor,
        resetTagTypeForm,
        startEditTagType,
        handleTagTypeSubmit,
        createTagMutation,
        assignTypeMutation,
        mergeMutation,
        deleteTagMutation,
        deleteTagTypeMutation,
    } = useTagsData();

    // Create-tag form state
    const [showCreate, setShowCreate] = useState(false);
    const [newTagName, setNewTagName] = useState("");
    const [newTagTypeId, setNewTagTypeId] = useState<string>("");

    // Merge dialog state
    const [mergeSource, setMergeSource] = useState<Tag | null>(null);
    const [mergeTargetId, setMergeTargetId] = useState<string>("");

    // Delete confirmation state
    const [deleteTagTarget, setDeleteTagTarget] = useState<{ id: string; name: string } | null>(null);
    const [deleteTagTypeTarget, setDeleteTagTypeTarget] = useState<{ id: string; name: string } | null>(null);

    const searchRef = useRef<HTMLInputElement>(null);

    // `/` to focus search — skip when typing in an input.
    useEffect(() => {
        function onKey(e: KeyboardEvent) {
            const target = e.target as HTMLElement | null;
            if (target?.tagName === "INPUT" || target?.tagName === "TEXTAREA" || target?.isContentEditable) return;
            if (e.key === "/" && !e.metaKey && !e.ctrlKey) {
                e.preventDefault();
                searchRef.current?.focus();
            }
        }
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, []);

    function handleCreateTagSubmit(e: React.FormEvent) {
        e.preventDefault();
        if (!newTagName.trim()) return;
        createTagMutation.mutate(
            { name: newTagName.trim(), ...(newTagTypeId ? { tagTypeId: newTagTypeId } : {}) },
            {
                onSuccess: () => {
                    setShowCreate(false);
                    setNewTagName("");
                    setNewTagTypeId("");
                }
            }
        );
    }

    const mergeCandidates = useMemo(
        () => tags.filter(t => t.id !== mergeSource?.id),
        [tags, mergeSource]
    );

    return (
        <PageContainer>
            <PageHeader
                icon={TagsIcon}
                title="Tags"
                count={tags.length}
            />

            <div className="grid gap-6 lg:grid-cols-[1fr_220px]">
                {/* Main content */}
                <div>
                    {/* Toolbar: search / sort / unused / create */}
                    <div className="mb-4 flex flex-wrap items-center gap-2">
                        <div className="relative min-w-0 flex-1">
                            <Search className="text-muted-foreground absolute top-1/2 left-3 size-4 -translate-y-1/2" />
                            <input
                                ref={searchRef}
                                type="text"
                                placeholder="Filter tags or types...  ( / )"
                                value={search}
                                onChange={e => setSearch(e.target.value)}
                                className="bg-background focus:ring-ring w-full rounded-lg border py-2 pl-9 pr-3 text-sm focus:ring-2 focus:outline-none"
                            />
                            {search && (
                                <button onClick={() => setSearch("")} className="text-muted-foreground absolute top-1/2 right-3 -translate-y-1/2">
                                    <X className="size-3.5" />
                                </button>
                            )}
                        </div>

                        <Select value={sortMode} onValueChange={v => { if (v) setSortMode(v as SortMode); }}>
                            <SelectTrigger size="sm" className="w-[150px]">
                                <ArrowUpDown className="size-3.5 opacity-70" />
                                <SelectValue />
                            </SelectTrigger>
                            <SelectPopup>
                                <SelectItem value="alpha">Alphabetical</SelectItem>
                                <SelectItem value="usage">Most used</SelectItem>
                            </SelectPopup>
                        </Select>

                        <Button
                            variant={unusedOnly ? "default" : "ghost"}
                            size="sm"
                            onClick={() => setUnusedOnly(u => !u)}
                            disabled={unusedCount === 0}
                            title={unusedCount === 0 ? "No unused tags" : "Show only unused tags"}
                        >
                            Unused
                            <span className="text-muted-foreground ml-1 text-xs tabular-nums">{unusedCount}</span>
                        </Button>

                        {!showCreate && (
                            <Button size="sm" onClick={() => setShowCreate(true)}>
                                <Plus className="size-3.5" />
                                New tag
                            </Button>
                        )}
                    </div>

                    {/* Inline create form */}
                    {showCreate && (
                        <form onSubmit={handleCreateTagSubmit} className="mb-4 rounded-lg border p-3 space-y-2">
                            <div className="flex gap-2">
                                <input
                                    type="text"
                                    placeholder="Tag name"
                                    value={newTagName}
                                    onChange={e => setNewTagName(e.target.value)}
                                    autoFocus
                                    required
                                    className="bg-background focus:ring-ring flex-1 rounded-md border px-2.5 py-1.5 text-sm focus:ring-2 focus:outline-none"
                                />
                                <select
                                    value={newTagTypeId}
                                    onChange={e => setNewTagTypeId(e.target.value)}
                                    className="bg-background rounded-md border px-2.5 py-1.5 text-sm"
                                >
                                    <option value="">No type</option>
                                    {tagTypes.map(tt => (
                                        <option key={tt.id} value={tt.id}>{tt.name}</option>
                                    ))}
                                </select>
                            </div>
                            <div className="flex justify-end gap-1.5">
                                <Button
                                    type="button"
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => { setShowCreate(false); setNewTagName(""); setNewTagTypeId(""); }}
                                >
                                    Cancel
                                </Button>
                                <Button type="submit" size="sm" disabled={!newTagName.trim() || createTagMutation.isPending}>
                                    {createTagMutation.isPending ? "Creating..." : "Create"}
                                </Button>
                            </div>
                        </form>
                    )}

                    {/* Tag groups */}
                    {tagsQuery.isLoading ? (
                        <PageLoading count={6} />
                    ) : filteredTags.length === 0 ? (
                        search || unusedOnly ? (
                            <div className="flex flex-col items-center gap-2 py-12">
                                <TagsIcon className="text-muted-foreground/30 size-8" />
                                <p className="text-muted-foreground text-sm">
                                    {unusedOnly ? "No unused tags" : "No tags match your search"}
                                </p>
                            </div>
                        ) : (
                            <PageEmpty
                                icon={TagIcon}
                                title="No tags yet"
                                description="Tags help categorize and filter your chunks."
                                action={<Button onClick={() => setShowCreate(true)}>Create Tag</Button>}
                            />
                        )
                    ) : (
                        <div className="space-y-6">
                            {sortedGroups.map(([key, { tagType, tags: groupTags }]) => (
                                <div key={key}>
                                    <div className="mb-2 flex items-center gap-2">
                                        {tagType ? (
                                            <>
                                                <div
                                                    className="size-3 rounded-full"
                                                    style={{ backgroundColor: tagType.color }}
                                                />
                                                <span className="text-sm font-semibold">{tagType.name}</span>
                                            </>
                                        ) : (
                                            <>
                                                <div className="border-muted-foreground/50 size-3 rounded-full border" />
                                                <span className="text-muted-foreground text-sm font-semibold">Uncategorized</span>
                                            </>
                                        )}
                                        <span className="text-muted-foreground text-xs">({groupTags.length})</span>
                                    </div>
                                    <div className="flex flex-wrap gap-2">
                                        {groupTags.map(t => (
                                            <TagPill
                                                key={t.id}
                                                tag={t}
                                                tagTypes={tagTypes}
                                                isRenaming={renamingId === t.id}
                                                renameValue={renameValue}
                                                setRenameValue={setRenameValue}
                                                onStartRename={() => startRename(t)}
                                                onCommitRename={() => commitRename(t)}
                                                onCancelRename={cancelRename}
                                                onAssignType={typeId => assignTypeMutation.mutate({ id: t.id, tagTypeId: typeId })}
                                                onMerge={() => setMergeSource(t)}
                                                onDelete={() => setDeleteTagTarget({ id: t.id, name: t.name })}
                                            />
                                        ))}
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </div>

                {/* Sidebar: Tag Types */}
                <div>
                    <div className="mb-3 flex items-center justify-between">
                        <div className="flex items-center gap-2">
                            <Palette className="text-muted-foreground size-4" />
                            <h2 className="text-sm font-semibold">Tag Types</h2>
                        </div>
                        {!showTagTypeForm && (
                            <Button variant="ghost" size="sm" onClick={() => setShowTagTypeForm(true)}>
                                <Plus className="size-3.5" />
                            </Button>
                        )}
                    </div>

                    {showTagTypeForm && (
                        <form onSubmit={handleTagTypeSubmit} className="mb-4 rounded-lg border p-3 space-y-2">
                            <div className="flex gap-2">
                                <input
                                    type="text"
                                    placeholder="Type name"
                                    value={ttName}
                                    onChange={e => setTtName(e.target.value)}
                                    required
                                    className="bg-background focus:ring-ring flex-1 rounded-md border px-2.5 py-1.5 text-sm focus:ring-2 focus:outline-none"
                                />
                                <input
                                    type="color"
                                    value={ttColor}
                                    onChange={e => setTtColor(e.target.value)}
                                    className="size-8 cursor-pointer rounded border p-0.5"
                                />
                            </div>
                            <div className="flex justify-end gap-1.5">
                                <Button type="button" variant="ghost" size="sm" onClick={resetTagTypeForm}>Cancel</Button>
                                <Button type="submit" size="sm" disabled={!ttName.trim()}>
                                    {editingTagType ? "Save" : "Create"}
                                </Button>
                            </div>
                        </form>
                    )}

                    {tagTypesQuery.isLoading ? (
                        <PageLoading count={3} />
                    ) : tagTypes.length === 0 ? (
                        <p className="text-muted-foreground text-xs">No tag types yet.</p>
                    ) : (
                        <div className="space-y-1">
                            {tagTypes.map(tt => {
                                const count = tags.filter(t => t.tagTypeId === tt.id).length;
                                return (
                                    <div
                                        key={tt.id}
                                        className="group flex items-center gap-2 rounded-md px-2.5 py-2 transition-colors hover:bg-muted/50"
                                    >
                                        <div
                                            className="size-3 shrink-0 rounded-full"
                                            style={{ backgroundColor: tt.color }}
                                        />
                                        <span className="flex-1 text-sm">{tt.name}</span>
                                        <span className="text-muted-foreground text-xs tabular-nums">{count}</span>
                                        <div className="flex gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                                            <button
                                                onClick={() => startEditTagType(tt)}
                                                className="text-muted-foreground hover:text-foreground"
                                            >
                                                <Pencil className="size-3" />
                                            </button>
                                            <button
                                                onClick={() => setDeleteTagTypeTarget({ id: tt.id, name: tt.name })}
                                                className="text-muted-foreground hover:text-destructive"
                                            >
                                                <Trash2 className="size-3" />
                                            </button>
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    )}
                </div>
            </div>

            {/* Merge dialog */}
            <Dialog open={mergeSource !== null} onOpenChange={open => { if (!open) { setMergeSource(null); setMergeTargetId(""); } }}>
                <DialogContent className="max-w-md">
                    <DialogHeader>
                        <DialogTitle>Merge tag</DialogTitle>
                        <DialogDescription>
                            All {mergeSource?.chunkCount ?? 0} chunks tagged
                            {" "}<span className="font-medium text-foreground">{mergeSource?.name}</span>{" "}
                            will be re-tagged with the target. The source tag is then deleted. This cannot be undone.
                        </DialogDescription>
                    </DialogHeader>
                    <div className="space-y-2 py-2">
                        <label className="text-sm font-medium">Merge into…</label>
                        <select
                            value={mergeTargetId}
                            onChange={e => setMergeTargetId(e.target.value)}
                            className="bg-background w-full rounded-md border px-2.5 py-2 text-sm"
                        >
                            <option value="">Choose a target tag…</option>
                            {mergeCandidates.map(t => (
                                <option key={t.id} value={t.id}>
                                    {t.name} {t.chunkCount > 0 ? `· ${t.chunkCount}` : ""}
                                </option>
                            ))}
                        </select>
                    </div>
                    <DialogFooter>
                        <Button
                            variant="ghost"
                            onClick={() => { setMergeSource(null); setMergeTargetId(""); }}
                        >
                            Cancel
                        </Button>
                        <Button
                            variant="destructive"
                            disabled={!mergeTargetId || mergeMutation.isPending}
                            onClick={() => {
                                if (mergeSource && mergeTargetId) {
                                    mergeMutation.mutate(
                                        { sourceId: mergeSource.id, targetId: mergeTargetId },
                                        { onSuccess: () => { setMergeSource(null); setMergeTargetId(""); } }
                                    );
                                }
                            }}
                        >
                            {mergeMutation.isPending ? "Merging..." : "Merge"}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            <ConfirmDialog
                open={deleteTagTarget !== null}
                onOpenChange={(open) => { if (!open) setDeleteTagTarget(null); }}
                title="Delete tag"
                description={deleteTagTarget ? `Delete tag "${deleteTagTarget.name}"? Chunk associations will be removed.` : ""}
                confirmLabel="Delete"
                confirmVariant="destructive"
                onConfirm={() => {
                    if (deleteTagTarget) {
                        deleteTagMutation.mutate(deleteTagTarget.id);
                        setDeleteTagTarget(null);
                    }
                }}
                loading={deleteTagMutation.isPending}
            />

            <ConfirmDialog
                open={deleteTagTypeTarget !== null}
                onOpenChange={(open) => { if (!open) setDeleteTagTypeTarget(null); }}
                title="Delete tag type"
                description={deleteTagTypeTarget ? `Delete tag type "${deleteTagTypeTarget.name}"? Tags in this type will become uncategorized.` : ""}
                confirmLabel="Delete"
                confirmVariant="destructive"
                onConfirm={() => {
                    if (deleteTagTypeTarget) {
                        deleteTagTypeMutation.mutate(deleteTagTypeTarget.id);
                        setDeleteTagTypeTarget(null);
                    }
                }}
                loading={deleteTagTypeMutation.isPending}
            />
        </PageContainer>
    );
}
