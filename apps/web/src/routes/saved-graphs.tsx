import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useActiveSpace } from "@/features/spaces/use-active-space";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

type SavedGraph = { id: string; name: string; description: string | null; chunkIds: string[] };
type Chunk = { id: string; title: string };

export const Route = createFileRoute("/saved-graphs")({ component: SavedGraphsPage });

function SavedGraphsPage() {
    const { spaceId } = useActiveSpace();
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const [name, setName] = useState("");
    const [description, setDescription] = useState("");
    const [chunkIds, setChunkIds] = useState<string[]>([]);
    const [chunkSearch, setChunkSearch] = useState("");
    const [editing, setEditing] = useState<string | null>(null);
    const [editName, setEditName] = useState("");
    const [deleting, setDeleting] = useState<SavedGraph | null>(null);

    const graphsQuery = useQuery({
        queryKey: ["saved-graphs", spaceId],
        queryFn: async () => unwrapEden(await api.api["saved-graphs"].get({ query: spaceId ? { spaceId } : {} })) as SavedGraph[]
    });
    const chunksQuery = useQuery({
        queryKey: ["saved-graph-chunks", spaceId],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { limit: "1000", ...(spaceId ? { codebaseId: spaceId } : {}) } }))
    });
    const chunks = (chunksQuery.data?.chunks ?? []) as Chunk[];
    const selected = new Set(chunkIds);
    const choices = chunks
        .filter(chunk => !selected.has(chunk.id) && chunk.title.toLowerCase().includes(chunkSearch.toLowerCase()))
        .slice(0, 30);

    const createGraph = useMutation({
        mutationFn: async () =>
            unwrapEden(
                await api.api["saved-graphs"].post({
                    name: name.trim(),
                    description: description.trim(),
                    chunkIds,
                    positions: {},
                    layoutAlgorithm: "force",
                    spaceId
                })
            ),
        onSuccess: async graph => {
            await queryClient.invalidateQueries({ queryKey: ["saved-graphs"] });
            toast.success("Graph saved");
            void navigate({ to: "/graph/$graphId", params: { graphId: graph.id } });
        },
        onError: () => toast.error("Could not save graph")
    });
    const renameGraph = useMutation({
        mutationFn: async (id: string) => unwrapEden(await api.api["saved-graphs"]({ id }).patch({ name: editName.trim() })),
        onSuccess: async () => {
            setEditing(null);
            await queryClient.invalidateQueries({ queryKey: ["saved-graphs"] });
            toast.success("Graph renamed");
        },
        onError: () => toast.error("Could not rename graph")
    });
    const deleteGraph = useMutation({
        mutationFn: async (id: string) => unwrapEden(await api.api["saved-graphs"]({ id }).delete()),
        onSuccess: async () => {
            setDeleting(null);
            await queryClient.invalidateQueries({ queryKey: ["saved-graphs"] });
            toast.success("Graph deleted");
        },
        onError: () => toast.error("Could not delete graph")
    });

    return (
        <div className="container mx-auto max-w-5xl space-y-8 px-4 py-8">
            <div className="flex items-center justify-between gap-4">
                <div>
                    <h1 className="text-2xl font-bold">Saved Graphs</h1>
                    <p className="text-muted-foreground text-sm">Keep focused maps of your chunks.</p>
                </div>
                <Button variant="outline" render={<Link to="/graph" />}>
                    Open graph
                </Button>
            </div>
            <section className="space-y-4 rounded-lg border p-5" aria-label="Create saved graph">
                <h2 className="text-lg font-semibold">Create saved graph</h2>
                <div>
                    <label htmlFor="graph-name" className="mb-1 block text-sm font-medium">
                        Name
                    </label>
                    <Input id="graph-name" value={name} onChange={event => setName(event.target.value)} maxLength={200} />
                </div>
                <div>
                    <label htmlFor="graph-description" className="mb-1 block text-sm font-medium">
                        Description
                    </label>
                    <Input id="graph-description" value={description} onChange={event => setDescription(event.target.value)} />
                </div>
                <div>
                    <label htmlFor="graph-chunk-search" className="mb-1 block text-sm font-medium">
                        Chunks
                    </label>
                    <Input
                        id="graph-chunk-search"
                        value={chunkSearch}
                        onChange={event => setChunkSearch(event.target.value)}
                        placeholder="Search chunks"
                    />
                    <div className="mt-2 flex flex-wrap gap-2">
                        {chunkIds.map(id => (
                            <button
                                key={id}
                                type="button"
                                className="bg-muted rounded px-2 py-1 text-sm"
                                onClick={() => setChunkIds(ids => ids.filter(value => value !== id))}
                                aria-label={`Remove ${chunks.find(chunk => chunk.id === id)?.title ?? "chunk"}`}
                            >
                                {chunks.find(chunk => chunk.id === id)?.title ?? id} ×
                            </button>
                        ))}
                    </div>
                    {chunksQuery.isError && (
                        <p role="alert" className="mt-2 text-sm text-red-600">
                            Could not load chunks.
                        </p>
                    )}
                    <div className="mt-2 max-h-40 overflow-y-auto">
                        {choices.map(chunk => (
                            <button
                                key={chunk.id}
                                type="button"
                                className="hover:bg-muted block w-full rounded px-2 py-1 text-left text-sm"
                                onClick={() => setChunkIds(ids => [...ids, chunk.id])}
                            >
                                + {chunk.title}
                            </button>
                        ))}
                    </div>
                </div>
                <Button onClick={() => createGraph.mutate()} disabled={!name.trim() || chunkIds.length === 0 || createGraph.isPending}>
                    Save graph
                </Button>
            </section>
            <section aria-label="Saved graphs">
                <h2 className="mb-3 text-lg font-semibold">Your graphs</h2>
                {graphsQuery.isLoading && <p>Loading graphs…</p>}
                {graphsQuery.isError && <p role="alert">Could not load saved graphs.</p>}
                {graphsQuery.isSuccess && graphsQuery.data.length === 0 && <p className="text-muted-foreground">No saved graphs yet.</p>}
                <ul className="divide-y rounded-lg border">
                    {graphsQuery.data?.map(graph => (
                        <li key={graph.id} className="flex flex-wrap items-center gap-2 p-3">
                            {editing === graph.id ? (
                                <form
                                    className="flex min-w-0 flex-1 gap-2"
                                    onSubmit={event => {
                                        event.preventDefault();
                                        if (editName.trim()) renameGraph.mutate(graph.id);
                                    }}
                                >
                                    <Input aria-label="Graph name" value={editName} onChange={event => setEditName(event.target.value)} />
                                    <Button type="submit" size="sm" disabled={!editName.trim() || renameGraph.isPending}>
                                        Save
                                    </Button>
                                    <Button type="button" variant="ghost" size="sm" onClick={() => setEditing(null)}>
                                        Cancel
                                    </Button>
                                </form>
                            ) : (
                                <>
                                    <Link
                                        to="/graph/$graphId"
                                        params={{ graphId: graph.id }}
                                        className="min-w-0 flex-1 font-medium hover:underline"
                                    >
                                        {graph.name}
                                        <span className="text-muted-foreground ml-2 text-xs">{graph.chunkIds.length} chunks</span>
                                    </Link>
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        onClick={() => {
                                            setEditing(graph.id);
                                            setEditName(graph.name);
                                        }}
                                    >
                                        Rename
                                    </Button>
                                    <Button variant="destructive" size="sm" onClick={() => setDeleting(graph)}>
                                        Delete
                                    </Button>
                                </>
                            )}
                        </li>
                    ))}
                </ul>
            </section>
            <ConfirmDialog
                open={!!deleting}
                onOpenChange={open => {
                    if (!open) setDeleting(null);
                }}
                title="Delete saved graph?"
                description={`Delete ${deleting?.name ?? "this graph"}? Its chunks will remain.`}
                confirmLabel="Delete graph"
                confirmVariant="destructive"
                loading={deleteGraph.isPending}
                onConfirm={() => {
                    if (deleting) deleteGraph.mutate(deleting.id);
                }}
            />
        </div>
    );
}
