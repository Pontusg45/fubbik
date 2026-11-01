import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

type Chunk = { id: string; title: string };
export type LearningPath = { id: string; title: string; description: string | null; chunkIds: string[] };

export function LearningPathForm({ path }: { path?: LearningPath }) {
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const [title, setTitle] = useState(path?.title ?? "");
    const [description, setDescription] = useState(path?.description ?? "");
    const [chunkIds, setChunkIds] = useState<string[]>(path?.chunkIds ?? []);
    const [filter, setFilter] = useState("");
    const chunksQuery = useQuery({
        queryKey: ["learning-path-chunks"],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { limit: "1000" } }))
    });
    const chunks = (chunksQuery.data?.chunks ?? []) as Chunk[];
    const byId = new Map(chunks.map(chunk => [chunk.id, chunk]));
    const choices = chunks.filter(chunk => !chunkIds.includes(chunk.id) && chunk.title.toLowerCase().includes(filter.toLowerCase()));

    const save = useMutation({
        mutationFn: async () => {
            const body = { title: title.trim(), description: description.trim(), chunkIds };
            return path
                ? unwrapEden(await api.api["learning-paths"]({ id: path.id }).patch(body))
                : unwrapEden(await api.api["learning-paths"].post(body));
        },
        onSuccess: async saved => {
            await queryClient.invalidateQueries({ queryKey: ["learning-paths"] });
            await queryClient.invalidateQueries({ queryKey: ["learning-path", saved.id] });
            toast.success(path ? "Learning path updated" : "Learning path created");
            void navigate({ to: "/learn/$pathId", params: { pathId: saved.id } });
        },
        onError: () => toast.error("Could not save learning path")
    });

    function move(index: number, direction: -1 | 1) {
        const next = [...chunkIds];
        const swap = index + direction;
        if (swap < 0 || swap >= next.length) return;
        [next[index], next[swap]] = [next[swap]!, next[index]!];
        setChunkIds(next);
    }

    return (
        <form
            className="space-y-6"
            onSubmit={event => {
                event.preventDefault();
                save.mutate();
            }}
        >
            <div>
                <label htmlFor="path-title" className="mb-1 block text-sm font-medium">
                    Title
                </label>
                <Input id="path-title" value={title} onChange={event => setTitle(event.target.value)} maxLength={200} required />
            </div>
            <div>
                <label htmlFor="path-description" className="mb-1 block text-sm font-medium">
                    Description
                </label>
                <textarea
                    id="path-description"
                    value={description}
                    onChange={event => setDescription(event.target.value)}
                    className="border-input bg-background w-full rounded-md border p-2 text-sm"
                    rows={3}
                />
            </div>
            <div>
                <h2 className="mb-2 text-lg font-semibold">Reading order</h2>
                {chunkIds.length === 0 && <p className="text-muted-foreground text-sm">Add chunks to build this path.</p>}
                <ol className="space-y-2">
                    {chunkIds.map((id, index) => (
                        <li key={id} className="flex items-center gap-2 rounded-md border p-2">
                            <span className="text-muted-foreground w-6 text-sm">{index + 1}.</span>
                            <span className="min-w-0 flex-1 truncate text-sm">{byId.get(id)?.title ?? "Unavailable chunk"}</span>
                            <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                onClick={() => move(index, -1)}
                                disabled={index === 0}
                                aria-label={`Move ${byId.get(id)?.title ?? "chunk"} up`}
                            >
                                ↑
                            </Button>
                            <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                onClick={() => move(index, 1)}
                                disabled={index === chunkIds.length - 1}
                                aria-label={`Move ${byId.get(id)?.title ?? "chunk"} down`}
                            >
                                ↓
                            </Button>
                            <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={() => setChunkIds(ids => ids.filter(value => value !== id))}
                                aria-label={`Remove ${byId.get(id)?.title ?? "chunk"}`}
                            >
                                Remove
                            </Button>
                        </li>
                    ))}
                </ol>
            </div>
            <div>
                <label htmlFor="path-chunk-search" className="mb-1 block text-sm font-medium">
                    Add chunks
                </label>
                <Input
                    id="path-chunk-search"
                    value={filter}
                    onChange={event => setFilter(event.target.value)}
                    placeholder="Search chunks"
                />
                {chunksQuery.isLoading && <p className="text-muted-foreground mt-2 text-sm">Loading chunks…</p>}
                {chunksQuery.isError && (
                    <p role="alert" className="mt-2 text-sm text-red-600">
                        Could not load chunks.
                    </p>
                )}
                <div className="mt-2 max-h-48 space-y-1 overflow-y-auto">
                    {choices.slice(0, 30).map(chunk => (
                        <button
                            key={chunk.id}
                            type="button"
                            onClick={() => setChunkIds(ids => [...ids, chunk.id])}
                            className="hover:bg-muted block w-full rounded-md px-2 py-1 text-left text-sm"
                        >
                            + {chunk.title}
                        </button>
                    ))}
                </div>
            </div>
            <div className="flex gap-2">
                <Button type="submit" disabled={!title.trim() || save.isPending}>
                    {save.isPending ? "Saving…" : "Save learning path"}
                </Button>
                <Button
                    type="button"
                    variant="outline"
                    render={<Link to={path ? "/learn/$pathId" : "/learn"} params={path ? { pathId: path.id } : undefined} />}
                >
                    Cancel
                </Button>
            </div>
        </form>
    );
}
