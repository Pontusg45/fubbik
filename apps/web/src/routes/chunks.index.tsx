import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { Clock, FileText, Plus, Search, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Separator } from "@/components/ui/separator";
import { getChunkSize } from "@/features/chunks/chunk-size";
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
        tags: (search.tags as string) || undefined
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
    const { type, q, page, sort, tags } = Route.useSearch();
    const queryClient = useQueryClient();
    const [searchInput, setSearchInput] = useState(q ?? "");
    const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
    const limit = 20;
    const offset = ((page ?? 1) - 1) * limit;

    const chunksQuery = useQuery({
        queryKey: ["chunks-list", type, q, page, sort, tags],
        queryFn: async () => {
            try {
                return unwrapEden(
                    await api.api.chunks.get({
                        query: {
                            type,
                            search: q,
                            sort: sort as "newest" | "oldest" | "alpha" | "updated" | undefined,
                            tags,
                            limit: String(limit),
                            offset: String(offset)
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
        queryFn: async () => unwrapEden(await api.api.tags.get())
    });

    const activeTags = tags ? tags.split(",") : [];

    function toggleTag(tag: string) {
        const current = activeTags;
        const next = current.includes(tag)
            ? current.filter(t => t !== tag)
            : [...current, tag];
        updateSearch({ tags: next.length > 0 ? next.join(",") : undefined });
    }

    const chunks = chunksQuery.data?.chunks ?? [];
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

    function updateSearch(params: Partial<{ type: string; q: string; page: number; sort: string; tags: string }>) {
        navigate({
            search: {
                type: params.type !== undefined ? params.type : type,
                q: params.q !== undefined ? params.q : q,
                page: params.page ?? 1,
                sort: params.sort !== undefined ? params.sort : sort,
                tags: params.tags !== undefined ? params.tags : tags
            }
        });
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

            {/* Search & filters */}
            <div className="mb-6 flex flex-wrap items-center gap-3">
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
                <div className="flex gap-1">
                    <Button variant={!type ? "default" : "outline"} size="sm" onClick={() => updateSearch({ type: undefined })}>
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
                <select
                    value={sort ?? "newest"}
                    onChange={e => updateSearch({ sort: e.target.value === "newest" ? undefined : e.target.value })}
                    className="rounded-md border bg-background px-2 py-1.5 text-sm"
                >
                    <option value="newest">Newest</option>
                    <option value="oldest">Oldest</option>
                    <option value="alpha">A-Z</option>
                    <option value="updated">Recently Updated</option>
                </select>
            </div>

            {(tagsQuery.data ?? []).length > 0 && (
                <div className="mb-4 flex flex-wrap gap-1">
                    {(tagsQuery.data ?? []).map(({ tag, count }) => (
                        <Badge
                            key={tag}
                            variant={activeTags.includes(tag) ? "default" : "outline"}
                            size="sm"
                            className="cursor-pointer text-[10px]"
                            onClick={() => toggleTag(tag)}
                        >
                            {tag} ({count})
                        </Badge>
                    ))}
                </div>
            )}

            {/* Results */}
            <Card>
                {chunksQuery.isLoading ? (
                    <CardPanel className="p-8 text-center">
                        <p className="text-muted-foreground text-sm">Loading...</p>
                    </CardPanel>
                ) : chunks.length === 0 ? (
                    <CardPanel className="p-8 text-center">
                        <p className="text-muted-foreground text-sm">No chunks found.</p>
                    </CardPanel>
                ) : (
                    chunks.map((chunk, i) => (
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
                                <Link to="/chunks/$chunkId" params={{ chunkId: chunk.id }} className="flex flex-1 items-center justify-between gap-4">
                                    <div className="min-w-0">
                                        <p className="truncate text-sm font-medium">{chunk.title}</p>
                                        <div className="mt-1 flex items-center gap-2">
                                            <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                                {chunk.type}
                                            </Badge>
                                            {(chunk.tags as string[]).map(tag => (
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
                    ))
                )}
            </Card>

            {/* Pagination */}
            {totalPages > 1 && (
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
                <div className="fixed bottom-6 left-1/2 z-50 flex -translate-x-1/2 items-center gap-3 rounded-lg border bg-background px-4 py-2.5 shadow-lg">
                    <span className="text-sm font-medium">{selectedIds.size} selected</span>
                    <Button
                        variant="destructive"
                        size="sm"
                        onClick={handleBulkDelete}
                        disabled={bulkDeleteMutation.isPending}
                    >
                        <Trash2 className="size-3.5" />
                        Delete
                    </Button>
                    <Button variant="ghost" size="sm" onClick={() => setSelectedIds(new Set())}>
                        Cancel
                    </Button>
                </div>
            )}
        </div>
    );
}
