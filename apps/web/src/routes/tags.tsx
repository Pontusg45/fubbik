import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Palette, Pencil, Plus, Search, Tags as TagsIcon, Trash2, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

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

interface Tag {
    id: string;
    name: string;
    tagTypeId: string | null;
    tagTypeName: string | null;
    tagTypeColor: string | null;
}

interface TagType {
    id: string;
    name: string;
    color: string;
}

function TagsPage() {
    const queryClient = useQueryClient();
    const [search, setSearch] = useState("");
    const [deleteTagTarget, setDeleteTagTarget] = useState<{ id: string; name: string } | null>(null);
    const [showTagTypeForm, setShowTagTypeForm] = useState(false);
    const [editingTagType, setEditingTagType] = useState<TagType | null>(null);
    const [ttName, setTtName] = useState("");
    const [ttColor, setTtColor] = useState("#8b5cf6");

    const tagsQuery = useQuery({
        queryKey: ["tags"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.tags.get()) as Tag[];
            } catch {
                return [];
            }
        }
    });

    const tagTypesQuery = useQuery({
        queryKey: ["tag-types"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api["tag-types"].get()) as TagType[];
            } catch {
                return [];
            }
        }
    });

    const deleteTagMutation = useMutation({
        mutationFn: async (id: string) => unwrapEden(await api.api.tags({ id }).delete()),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            toast.success("Tag deleted");
        },
        onError: () => toast.error("Failed to delete tag")
    });

    const createTagTypeMutation = useMutation({
        mutationFn: async (body: { name: string; color: string }) => unwrapEden(await api.api["tag-types"].post(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["tag-types"] });
            resetTagTypeForm();
            toast.success("Tag type created");
        },
        onError: () => toast.error("Failed to create tag type")
    });

    const updateTagTypeMutation = useMutation({
        mutationFn: async ({ id, body }: { id: string; body: { name?: string; color?: string } }) =>
            unwrapEden(await api.api["tag-types"]({ id }).patch(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["tag-types"] });
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            resetTagTypeForm();
            toast.success("Tag type updated");
        },
        onError: () => toast.error("Failed to update tag type")
    });

    const deleteTagTypeMutation = useMutation({
        mutationFn: async (id: string) => unwrapEden(await api.api["tag-types"]({ id }).delete()),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["tag-types"] });
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            toast.success("Tag type deleted");
        },
        onError: () => toast.error("Failed to delete tag type")
    });

    function resetTagTypeForm() {
        setShowTagTypeForm(false);
        setEditingTagType(null);
        setTtName("");
        setTtColor("#8b5cf6");
    }

    function startEditTagType(tt: TagType) {
        setEditingTagType(tt);
        setTtName(tt.name);
        setTtColor(tt.color);
        setShowTagTypeForm(true);
    }

    function handleTagTypeSubmit(e: React.FormEvent) {
        e.preventDefault();
        if (!ttName.trim()) return;
        if (editingTagType) {
            updateTagTypeMutation.mutate({ id: editingTagType.id, body: { name: ttName.trim(), color: ttColor } });
        } else {
            createTagTypeMutation.mutate({ name: ttName.trim(), color: ttColor });
        }
    }

    const tags = Array.isArray(tagsQuery.data) ? tagsQuery.data : [];
    const tagTypes = Array.isArray(tagTypesQuery.data) ? tagTypesQuery.data : [];
    const filteredTags = search
        ? tags.filter(t => t.name.toLowerCase().includes(search.toLowerCase()))
        : tags;

    // Group tags by tag type
    const grouped = new Map<string, { tagType: { name: string; color: string; id: string } | null; tags: Tag[] }>();
    for (const t of filteredTags) {
        const key = t.tagTypeId ?? "__none__";
        if (!grouped.has(key)) {
            grouped.set(key, {
                tagType: t.tagTypeId ? { name: t.tagTypeName ?? "Unknown", color: t.tagTypeColor ?? "#888", id: t.tagTypeId } : null,
                tags: []
            });
        }
        grouped.get(key)!.tags.push(t);
    }

    // Sort: named groups first (alphabetical), then ungrouped
    const sortedGroups = [...grouped.entries()].sort((a, b) => {
        if (!a[1].tagType) return 1;
        if (!b[1].tagType) return -1;
        return a[1].tagType.name.localeCompare(b[1].tagType.name);
    });

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            {/* Header */}
            <div className="mb-6 flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <TagsIcon className="size-5" />
                    <h1 className="text-2xl font-bold tracking-tight">Tags</h1>
                    <Badge variant="secondary" className="ml-1">{tags.length}</Badge>
                </div>
            </div>

            <div className="grid gap-6 lg:grid-cols-[1fr_220px]">
                {/* Main content */}
                <div>
                    {/* Search */}
                    <div className="relative mb-4">
                        <Search className="text-muted-foreground absolute top-1/2 left-3 size-4 -translate-y-1/2" />
                        <input
                            type="text"
                            placeholder="Filter tags..."
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

                    {/* Tag groups */}
                    {tagsQuery.isLoading ? (
                        <p className="text-muted-foreground py-8 text-center text-sm">Loading tags...</p>
                    ) : filteredTags.length === 0 ? (
                        <div className="flex flex-col items-center gap-2 py-12">
                            <TagsIcon className="text-muted-foreground/30 size-8" />
                            <p className="text-muted-foreground text-sm">
                                {search ? "No tags match your search" : "No tags yet"}
                            </p>
                        </div>
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
                                                <span className="text-muted-foreground text-xs">({groupTags.length})</span>
                                            </>
                                        ) : (
                                            <>
                                                <span className="text-muted-foreground text-sm font-semibold">Uncategorized</span>
                                                <span className="text-muted-foreground text-xs">({groupTags.length})</span>
                                            </>
                                        )}
                                    </div>
                                    <div className="flex flex-wrap gap-2">
                                        {groupTags.map(t => (
                                            <div
                                                key={t.id}
                                                className="group flex items-center gap-1.5 rounded-lg border px-3 py-1.5 transition-colors hover:bg-muted/50"
                                            >
                                                {t.tagTypeColor && (
                                                    <div
                                                        className="size-2 rounded-full shrink-0"
                                                        style={{ backgroundColor: t.tagTypeColor }}
                                                    />
                                                )}
                                                <Link
                                                    to="/chunks"
                                                    search={{ tags: t.name } as any}
                                                    className="text-sm hover:underline"
                                                >
                                                    {t.name}
                                                </Link>
                                                <button
                                                    onClick={() => setDeleteTagTarget({ id: t.id, name: t.name })}
                                                    className="text-muted-foreground/0 group-hover:text-muted-foreground hover:text-destructive ml-1 transition-colors"
                                                >
                                                    <X className="size-3" />
                                                </button>
                                            </div>
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
                        <p className="text-muted-foreground text-xs">Loading...</p>
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
                                                onClick={() => {
                                                    if (confirm(`Delete tag type "${tt.name}"? Tags in this type will become uncategorized.`)) // TODO: replace with styled dialog
                                                        deleteTagTypeMutation.mutate(tt.id);
                                                }}
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

            <ConfirmDialog
                open={deleteTagTarget !== null}
                onOpenChange={(open) => { if (!open) setDeleteTagTarget(null); }}
                title="Delete tag"
                description={deleteTagTarget ? `Delete tag "${deleteTagTarget.name}"?` : ""}
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
        </div>
    );
}
