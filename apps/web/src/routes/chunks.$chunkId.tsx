import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, redirect, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, Bot, Calendar, Clock, Edit, Hash, History, Link2, Loader2, Network, Search, Sparkles, Trash2, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { Dialog, DialogHeader, DialogPopup, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";

export const Route = createFileRoute("/chunks/$chunkId")({
    component: ChunkDetail,
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            throw redirect({ to: "/login" });
        }
    }
});

function ChunkDetail() {
    const { chunkId } = Route.useParams();
    const navigate = useNavigate();
    const queryClient = useQueryClient();

    const { data, isLoading, error } = useQuery({
        queryKey: ["chunk", chunkId],
        queryFn: async () => {
            const { data, error } = await api.api.chunks({ id: chunkId }).get();
            if (error) throw new Error("Failed to load chunk");
            return data;
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async () => {
            const { error } = await api.api.chunks({ id: chunkId }).delete();
            if (error) throw new Error("Failed to delete chunk");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            toast.success("Chunk deleted");
            navigate({ to: "/dashboard" });
        },
        onError: () => {
            toast.error("Failed to delete chunk");
        }
    });

    if (isLoading) {
        return (
            <div className="container mx-auto max-w-3xl px-4 py-8">
                <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="mt-8 text-center">
                    <p className="text-muted-foreground">Loading...</p>
                </div>
            </div>
        );
    }

    if (error || !data?.chunk) {
        return (
            <div className="container mx-auto max-w-3xl px-4 py-8">
                <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="mt-8 text-center">
                    <p className="text-muted-foreground">Chunk "{chunkId}" not found.</p>
                </div>
            </div>
        );
    }

    const chunk = data.chunk;
    const connections = data.connections ?? [];

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="flex gap-2">
                    <Button variant="outline" size="sm" render={<Link to="/chunks/$chunkId/edit" params={{ chunkId }} />}>
                        <Edit className="size-3.5" />
                        Edit
                    </Button>
                    <Button
                        variant="outline"
                        size="sm"
                        className="text-destructive"
                        onClick={() => deleteMutation.mutate()}
                        disabled={deleteMutation.isPending}
                    >
                        <Trash2 className="size-3.5" />
                        {deleteMutation.isPending ? "Deleting..." : "Delete"}
                    </Button>
                </div>
            </div>

            <div className="mb-6">
                <div className="mb-2 flex items-center gap-2">
                    <Badge variant="secondary" className="font-mono text-xs">
                        {chunk.type}
                    </Badge>
                    <span className="text-muted-foreground flex items-center gap-1 font-mono text-xs">
                        <Hash className="size-3" />
                        {chunk.id.slice(0, 8)}
                    </span>
                </div>
                <h1 className="text-2xl font-bold tracking-tight">{chunk.title}</h1>
                <div className="text-muted-foreground mt-2 flex items-center gap-4 text-xs">
                    <span className="flex items-center gap-1">
                        <Calendar className="size-3" />
                        Created {new Date(chunk.createdAt).toLocaleDateString()}
                    </span>
                    <span className="flex items-center gap-1">
                        <Clock className="size-3" />
                        Updated {new Date(chunk.updatedAt).toLocaleDateString()}
                    </span>
                </div>
            </div>

            <div className="mb-2 flex flex-wrap gap-1.5">
                {(chunk.tags as string[]).map(tag => (
                    <Badge key={tag} variant="outline" size="sm">
                        {tag}
                    </Badge>
                ))}
            </div>

            <Separator className="my-6" />

            <div className="prose prose-invert prose-sm max-w-none">
                {chunk.content.split("\n\n").map((block, i) => {
                    if (block.startsWith("## ")) {
                        return (
                            <h2 key={i} className="mt-6 mb-2 text-base font-semibold">
                                {block.replace("## ", "")}
                            </h2>
                        );
                    }
                    if (block.startsWith("- ")) {
                        return (
                            <ul key={i} className="text-muted-foreground list-disc space-y-1 pl-5 text-sm">
                                {block.split("\n").map((line, j) => (
                                    <li key={j}>{line.replace(/^- /, "")}</li>
                                ))}
                            </ul>
                        );
                    }
                    return (
                        <p key={i} className="text-muted-foreground text-sm leading-relaxed">
                            {block}
                        </p>
                    );
                })}
            </div>

            <Separator className="my-6" />
            <AiSection chunkId={chunkId} />

            <Separator className="my-6" />
            <Card>
                <CardHeader>
                    <div className="flex items-center justify-between">
                        <div>
                            <CardTitle className="flex items-center gap-2 text-sm">
                                <Network className="size-4" />
                                Connections
                            </CardTitle>
                            <CardDescription>{connections.length} linked chunks</CardDescription>
                        </div>
                        <LinkChunkDialog chunkId={chunkId} />
                    </div>
                </CardHeader>
                {connections.length > 0 && (
                    <CardPanel className="space-y-2 pt-0">
                        {connections.map(conn => {
                            const linkedId = conn.sourceId === chunkId ? conn.targetId : conn.sourceId;
                            return (
                                <div
                                    key={conn.id}
                                    className="hover:bg-muted flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                                >
                                    <Link
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: linkedId }}
                                        className="flex-1 font-medium"
                                    >
                                        {conn.title ?? linkedId}
                                    </Link>
                                    <div className="flex items-center gap-2">
                                        <Badge variant="outline" size="sm" className="text-[10px]">
                                            {conn.relation}
                                        </Badge>
                                        <DeleteConnectionButton connectionId={conn.id} chunkId={chunkId} />
                                    </div>
                                </div>
                            );
                        })}
                    </CardPanel>
                )}
            </Card>

            <Separator className="my-6" />
            <VersionHistory chunkId={chunkId} />
        </div>
    );
}

function VersionHistory({ chunkId }: { chunkId: string }) {
    const [open, setOpen] = useState(false);

    const historyQuery = useQuery({
        queryKey: ["chunk-history", chunkId],
        queryFn: async () => {
            const { data, error } = await api.api.chunks({ id: chunkId }).history.get();
            if (error) throw new Error("Failed to load history");
            return data as Array<{
                id: string;
                version: number;
                title: string;
                content: string;
                type: string;
                tags: string[];
                createdAt: string;
            }>;
        },
        enabled: open
    });

    const versions = historyQuery.data ?? [];

    return (
        <Card>
            <CardHeader>
                <button
                    type="button"
                    onClick={() => setOpen(!open)}
                    className="flex w-full items-center justify-between"
                >
                    <CardTitle className="flex items-center gap-2 text-sm">
                        <History className="size-4" />
                        Version History
                    </CardTitle>
                    <Badge variant="secondary" size="sm">
                        {open ? "Hide" : "Show"}
                    </Badge>
                </button>
            </CardHeader>
            {open && (
                <CardPanel className="space-y-2 pt-0">
                    {historyQuery.isLoading && (
                        <p className="text-muted-foreground text-sm">Loading history...</p>
                    )}
                    {versions.length === 0 && !historyQuery.isLoading && (
                        <p className="text-muted-foreground text-sm">No previous versions</p>
                    )}
                    {versions.map(v => (
                        <div key={v.id} className="rounded-md border px-3 py-2">
                            <div className="flex items-center justify-between">
                                <span className="text-sm font-medium">v{v.version}: {v.title}</span>
                                <span className="text-muted-foreground text-xs">
                                    {new Date(v.createdAt).toLocaleString()}
                                </span>
                            </div>
                            <p className="text-muted-foreground mt-1 line-clamp-2 text-xs">{v.content}</p>
                        </div>
                    ))}
                </CardPanel>
            )}
        </Card>
    );
}

function DeleteConnectionButton({ connectionId, chunkId }: { connectionId: string; chunkId: string }) {
    const queryClient = useQueryClient();

    const deleteMutation = useMutation({
        mutationFn: async () => {
            const { error } = await api.api.connections({ id: connectionId }).delete();
            if (error) throw new Error("Failed to delete connection");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            toast.success("Connection removed");
        },
        onError: () => {
            toast.error("Failed to remove connection");
        }
    });

    return (
        <button
            type="button"
            onClick={e => {
                e.preventDefault();
                e.stopPropagation();
                deleteMutation.mutate();
            }}
            disabled={deleteMutation.isPending}
            className="text-muted-foreground hover:text-destructive rounded p-0.5 transition-colors"
            aria-label="Remove connection"
        >
            <X className="size-3.5" />
        </button>
    );
}

function AiSection({ chunkId }: { chunkId: string }) {
    const [summary, setSummary] = useState<string | null>(null);
    const [suggestions, setSuggestions] = useState<{ id: string; relation: string }[] | null>(null);

    const summarizeMutation = useMutation({
        mutationFn: async () => {
            const { data, error } = await api.api.ai.summarize.post({ chunkId });
            if (error) throw new Error("Failed to summarize");
            return data as { summary: string };
        },
        onSuccess: data => {
            setSummary(data.summary);
        },
        onError: () => {
            toast.error("Failed to summarize chunk");
        }
    });

    const suggestMutation = useMutation({
        mutationFn: async () => {
            const { data, error } = await api.api.ai["suggest-connections"].post({ chunkId });
            if (error) throw new Error("Failed to suggest connections");
            return data as { id: string; relation: string }[];
        },
        onSuccess: data => {
            setSuggestions(data);
        },
        onError: () => {
            toast.error("Failed to suggest connections");
        }
    });

    return (
        <Card>
            <CardHeader>
                <CardTitle className="flex items-center gap-2 text-sm">
                    <Bot className="size-4" />
                    AI Tools
                </CardTitle>
                <CardDescription>Use AI to analyze this chunk</CardDescription>
            </CardHeader>
            <CardPanel className="space-y-4 pt-0">
                <div className="flex gap-2">
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => summarizeMutation.mutate()}
                        disabled={summarizeMutation.isPending}
                    >
                        {summarizeMutation.isPending ? (
                            <Loader2 className="size-3.5 animate-spin" />
                        ) : (
                            <Sparkles className="size-3.5" />
                        )}
                        Summarize
                    </Button>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => suggestMutation.mutate()}
                        disabled={suggestMutation.isPending}
                    >
                        {suggestMutation.isPending ? (
                            <Loader2 className="size-3.5 animate-spin" />
                        ) : (
                            <Network className="size-3.5" />
                        )}
                        Suggest Connections
                    </Button>
                </div>

                {summary && (
                    <div className="bg-muted rounded-md p-3">
                        <p className="text-muted-foreground mb-1 text-xs font-medium">Summary</p>
                        <p className="text-sm">{summary}</p>
                    </div>
                )}

                {suggestions && suggestions.length > 0 && (
                    <div className="bg-muted rounded-md p-3">
                        <p className="text-muted-foreground mb-2 text-xs font-medium">Suggested Connections</p>
                        <div className="space-y-1">
                            {suggestions.map(s => (
                                <div key={s.id} className="flex items-center justify-between text-sm">
                                    <Link
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: s.id }}
                                        className="text-primary hover:underline"
                                    >
                                        {s.id.slice(0, 8)}...
                                    </Link>
                                    <Badge variant="outline" size="sm" className="text-[10px]">
                                        {s.relation}
                                    </Badge>
                                </div>
                            ))}
                        </div>
                    </div>
                )}

                {suggestions && suggestions.length === 0 && (
                    <p className="text-muted-foreground text-sm">No connection suggestions found.</p>
                )}
            </CardPanel>
        </Card>
    );
}

function LinkChunkDialog({ chunkId }: { chunkId: string }) {
    const [open, setOpen] = useState(false);
    const [search, setSearch] = useState("");
    const [relation, setRelation] = useState("related");
    const queryClient = useQueryClient();

    const { data: searchResults } = useQuery({
        queryKey: ["chunks", "search", search],
        queryFn: async () => {
            if (!search.trim()) return { chunks: [] };
            const { data, error } = await api.api.chunks.get({ query: { search, limit: "10" } });
            if (error) throw new Error("Failed to search chunks");
            return data;
        },
        enabled: open && search.trim().length > 0
    });

    const createMutation = useMutation({
        mutationFn: async (targetId: string) => {
            const { error } = await api.api.connections.post({
                sourceId: chunkId,
                targetId,
                relation
            });
            if (error) throw new Error("Failed to create connection");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            toast.success("Connection created");
            setSearch("");
            setOpen(false);
        },
        onError: () => {
            toast.error("Failed to create connection");
        }
    });

    const filteredResults = (searchResults?.chunks ?? []).filter(c => c.id !== chunkId);

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogTrigger render={<Button variant="outline" size="sm" />}>
                <Link2 className="size-3.5" />
                Link Chunk
            </DialogTrigger>
            <DialogPopup>
                <DialogHeader>
                    <DialogTitle>Link Chunk</DialogTitle>
                </DialogHeader>
                <div className="space-y-4 px-6 pb-6">
                    <div className="space-y-2">
                        <label className="text-muted-foreground text-xs font-medium">Relation</label>
                        <Input
                            value={relation}
                            onChange={e => setRelation((e.target as HTMLInputElement).value)}
                            placeholder="e.g. related, depends-on, extends"
                        />
                    </div>
                    <div className="space-y-2">
                        <label className="text-muted-foreground text-xs font-medium">Search chunks</label>
                        <div className="relative">
                            <Search className="text-muted-foreground pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2" />
                            <Input
                                value={search}
                                onChange={e => setSearch((e.target as HTMLInputElement).value)}
                                placeholder="Type to search..."
                                className="pl-8"
                            />
                        </div>
                    </div>
                    {filteredResults.length > 0 && (
                        <div className="max-h-60 space-y-1 overflow-y-auto rounded-md border p-1">
                            {filteredResults.map(result => (
                                <button
                                    key={result.id}
                                    type="button"
                                    onClick={() => createMutation.mutate(result.id)}
                                    disabled={createMutation.isPending}
                                    className="hover:bg-muted flex w-full items-center justify-between rounded-md px-3 py-2 text-left text-sm transition-colors"
                                >
                                    <span className="font-medium">{result.title}</span>
                                    <Badge variant="secondary" size="sm" className="text-[10px]">
                                        {result.type}
                                    </Badge>
                                </button>
                            ))}
                        </div>
                    )}
                    {search.trim().length > 0 && filteredResults.length === 0 && (
                        <p className="text-muted-foreground py-4 text-center text-sm">No chunks found</p>
                    )}
                </div>
            </DialogPopup>
        </Dialog>
    );
}
