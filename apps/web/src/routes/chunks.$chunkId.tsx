import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, Calendar, Clock, Edit, FileText, Hash, Network, Star, Trash2 } from "lucide-react";
import { useEffect } from "react";
import { toast } from "sonner";

import { MarkdownRenderer } from "@/components/markdown-renderer";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { AiSection } from "@/features/chunks/ai-section";
import { getChunkSize } from "@/features/chunks/chunk-size";
import { DeleteConnectionButton } from "@/features/chunks/delete-connection-button";
import { LinkChunkDialog } from "@/features/chunks/link-chunk-dialog";
import { relationColor } from "@/features/chunks/relation-colors";
import { SplitChunkDialog } from "@/features/chunks/split-chunk-dialog";
import { useFavorites } from "@/features/chunks/use-favorites";
import { useRecentChunks } from "@/features/chunks/use-recent-chunks";
import { VersionHistory } from "@/features/chunks/version-history";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";

export const Route = createFileRoute("/chunks/$chunkId")({
    component: ChunkDetail,
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

function ChunkDetail() {
    const { chunkId } = Route.useParams();
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const { trackView } = useRecentChunks();
    const { toggleFavorite, isFavorite } = useFavorites();

    useEffect(() => {
        trackView(chunkId);
    }, [chunkId, trackView]);

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
    const outgoing = connections.filter(c => c.sourceId === chunkId);
    const incoming = connections.filter(c => c.sourceId !== chunkId);

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="flex gap-2">
                    <SplitChunkDialog
                        chunkId={chunkId}
                        title={chunk.title}
                        content={chunk.content}
                        type={chunk.type}
                        tags={[]}
                    />
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
                <div className="flex items-center gap-2">
                    <h1 className="text-2xl font-bold tracking-tight">{chunk.title}</h1>
                    <button
                        onClick={() => toggleFavorite(chunkId)}
                        className="text-muted-foreground transition-colors hover:text-yellow-500"
                        title={isFavorite(chunkId) ? "Remove from favorites" : "Add to favorites"}
                    >
                        <Star className={`size-4 ${isFavorite(chunkId) ? "fill-yellow-500 text-yellow-500" : ""}`} />
                    </button>
                </div>
                <div className="text-muted-foreground mt-2 flex flex-wrap items-center gap-4 text-xs">
                    <span className="flex items-center gap-1">
                        <Calendar className="size-3" />
                        Created {new Date(chunk.createdAt).toLocaleDateString()}
                    </span>
                    <span className="flex items-center gap-1">
                        <Clock className="size-3" />
                        Updated {new Date(chunk.updatedAt).toLocaleDateString()}
                    </span>
                    {(() => {
                        const size = getChunkSize(chunk.content);
                        return (
                            <span className="flex items-center gap-1" style={{ color: size.color }}>
                                <FileText className="size-3" />
                                {size.lines} lines · {size.chars.toLocaleString()} chars · {size.label}
                            </span>
                        );
                    })()}
                </div>
            </div>

            <div className="mb-2 flex flex-wrap gap-1.5">
                {/* tags are now in normalized tables, rendered via tag components */}
            </div>

            <Separator className="my-6" />

            <div className="prose dark:prose-invert prose-sm max-w-none">
                <MarkdownRenderer>{chunk.content}</MarkdownRenderer>
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
                {outgoing.length > 0 && (
                    <CardPanel className="space-y-2 pt-0">
                        <p className="text-muted-foreground text-xs font-medium">Links to</p>
                        {outgoing.map(conn => (
                            <div
                                key={conn.id}
                                className="hover:bg-muted flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                            >
                                <Link to="/chunks/$chunkId" params={{ chunkId: conn.targetId }} className="flex-1 font-medium">
                                    {conn.title ?? conn.targetId}
                                </Link>
                                <div className="flex items-center gap-2">
                                    <Badge
                                        variant="outline"
                                        size="sm"
                                        className="text-[10px]"
                                        style={{ borderColor: relationColor(conn.relation), color: relationColor(conn.relation) }}
                                    >
                                        {conn.relation}
                                    </Badge>
                                    <DeleteConnectionButton connectionId={conn.id} chunkId={chunkId} />
                                </div>
                            </div>
                        ))}
                    </CardPanel>
                )}
                {incoming.length > 0 && (
                    <CardPanel className="space-y-2 pt-0">
                        <p className="text-muted-foreground text-xs font-medium">Linked from</p>
                        {incoming.map(conn => (
                            <div
                                key={conn.id}
                                className="hover:bg-muted flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                            >
                                <Link to="/chunks/$chunkId" params={{ chunkId: conn.sourceId }} className="flex-1 font-medium">
                                    {conn.title ?? conn.sourceId}
                                </Link>
                                <div className="flex items-center gap-2">
                                    <Badge
                                        variant="outline"
                                        size="sm"
                                        className="text-[10px]"
                                        style={{ borderColor: relationColor(conn.relation), color: relationColor(conn.relation) }}
                                    >
                                        {conn.relation}
                                    </Badge>
                                    <DeleteConnectionButton connectionId={conn.id} chunkId={chunkId} />
                                </div>
                            </div>
                        ))}
                    </CardPanel>
                )}
                {connections.length === 0 && (
                    <CardPanel className="pt-0">
                        <p className="text-muted-foreground text-sm">No connections yet</p>
                    </CardPanel>
                )}
            </Card>

            <Separator className="my-6" />
            <VersionHistory chunkId={chunkId} />
        </div>
    );
}
