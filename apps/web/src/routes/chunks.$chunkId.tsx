import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, Bot, Calendar, Clock, Code, Edit, FileCode, FileText, Hash, Network, Scale, Star, Trash2 } from "lucide-react";
import { useEffect } from "react";
import { toast } from "sonner";

import { MarkdownRenderer } from "@/components/markdown-renderer";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { AiSection } from "@/features/chunks/ai-section";
import { ChunkLink } from "@/features/chunks/chunk-link";
import { getChunkSize } from "@/features/chunks/chunk-size";
import { DeleteConnectionButton } from "@/features/chunks/delete-connection-button";
import { LinkChunkDialog } from "@/features/chunks/link-chunk-dialog";
import { RelatedChunks } from "@/features/chunks/related-chunks";
import { relationColor } from "@/features/chunks/relation-colors";
import { SplitChunkDialog } from "@/features/chunks/split-chunk-dialog";
import { SuggestedConnections } from "@/features/chunks/suggested-connections";
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

    const reviewMutation = useMutation({
        mutationFn: async (reviewStatus: "reviewed" | "approved") => {
            const { error } = await api.api.chunks({ id: chunkId }).patch({ reviewStatus });
            if (error) throw new Error("Failed to update review status");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            toast.success("Review status updated");
        },
        onError: () => {
            toast.error("Failed to update review status");
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
    const origin = (chunk as Record<string, unknown>).origin as string | undefined;
    const reviewStatus = (chunk as Record<string, unknown>).reviewStatus as string | undefined;
    const isAi = origin === "ai";
    const connections = data.connections ?? [];
    const outgoing = connections.filter(c => c.sourceId === chunkId);
    const incoming = connections.filter(c => c.sourceId !== chunkId);
    const appliesTo = (data as Record<string, unknown>).appliesTo as
        | Array<{ id: string; pattern: string; note?: string | null }>
        | undefined;
    const fileReferences = (data as Record<string, unknown>).fileReferences as
        | Array<{ id: string; path: string; anchor?: string | null; relation: string }>
        | undefined;
    const tags = ((data as Record<string, unknown>).tags as Array<{ id: string; name: string }> | undefined) ?? [];
    const rationale = (chunk as Record<string, unknown>).rationale as string | null | undefined;
    const alternatives = (chunk as Record<string, unknown>).alternatives as string[] | null | undefined;
    const consequences = (chunk as Record<string, unknown>).consequences as string | null | undefined;
    const hasDecisionContext = !!rationale || (alternatives && alternatives.length > 0) || !!consequences;

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
                    <Button
                        variant="outline"
                        size="sm"
                        render={<Link to="/graph" search={{ pathFrom: chunkId }} />}
                    >
                        <Network className="size-3.5" />
                        Find path
                    </Button>
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
                    {isAi && (
                        <Badge
                            variant="outline"
                            className={
                                reviewStatus === "draft"
                                    ? "border-yellow-500/30 bg-yellow-500/10 text-yellow-600"
                                    : reviewStatus === "reviewed"
                                      ? "border-blue-500/30 bg-blue-500/10 text-blue-600"
                                      : "border-green-500/30 bg-green-500/10 text-green-600"
                            }
                        >
                            <Bot className="mr-1 size-3" />
                            AI {reviewStatus === "draft" ? "Draft" : reviewStatus === "reviewed" ? "Reviewed" : "Approved"}
                        </Badge>
                    )}
                    <button
                        onClick={() => toggleFavorite(chunkId)}
                        className="text-muted-foreground transition-colors hover:text-yellow-500"
                        title={isFavorite(chunkId) ? "Remove from favorites" : "Add to favorites"}
                    >
                        <Star className={`size-4 ${isFavorite(chunkId) ? "fill-yellow-500 text-yellow-500" : ""}`} />
                    </button>
                </div>
                {isAi && reviewStatus !== "approved" && (
                    <div className="mt-2 flex items-center gap-2">
                        {reviewStatus === "draft" && (
                            <Button
                                variant="outline"
                                size="sm"
                                onClick={() => reviewMutation.mutate("reviewed")}
                                disabled={reviewMutation.isPending}
                            >
                                Mark Reviewed
                            </Button>
                        )}
                        <Button
                            variant="outline"
                            size="sm"
                            onClick={() => reviewMutation.mutate("approved")}
                            disabled={reviewMutation.isPending}
                        >
                            Mark Approved
                        </Button>
                    </div>
                )}
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

            {appliesTo && appliesTo.length > 0 && (
                <>
                    <Separator className="my-6" />
                    <div>
                        <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
                            <Code className="size-4" />
                            Applies To
                        </h2>
                        <div className="flex flex-wrap gap-2">
                            {appliesTo.map(item => (
                                <Badge key={item.id} variant="outline" className="font-mono text-xs">
                                    {item.pattern}
                                    {item.note && (
                                        <span className="text-muted-foreground ml-1 font-sans">({item.note})</span>
                                    )}
                                </Badge>
                            ))}
                        </div>
                    </div>
                </>
            )}

            {fileReferences && fileReferences.length > 0 && (
                <>
                    <Separator className="my-6" />
                    <div>
                        <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
                            <FileCode className="size-4" />
                            File References
                        </h2>
                        <div className="space-y-2">
                            {fileReferences.map(ref => (
                                <div key={ref.id} className="flex items-center gap-2 text-sm">
                                    <code className="bg-muted rounded px-1.5 py-0.5 font-mono text-xs">{ref.path}</code>
                                    {ref.anchor && (
                                        <Badge variant="secondary" size="sm" className="text-[10px]">
                                            {ref.anchor}
                                        </Badge>
                                    )}
                                    <span className="text-muted-foreground text-xs">{ref.relation}</span>
                                </div>
                            ))}
                        </div>
                    </div>
                </>
            )}

            {hasDecisionContext && (
                <>
                    <Separator className="my-6" />
                    <div className="bg-muted/50 rounded-lg border p-4">
                        <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
                            <Scale className="size-4" />
                            Decision Context
                        </h2>
                        {rationale && (
                            <div className="mb-3">
                                <p className="text-muted-foreground mb-1 text-xs font-medium uppercase tracking-wide">Rationale</p>
                                <p className="text-sm">{rationale}</p>
                            </div>
                        )}
                        {alternatives && alternatives.length > 0 && (
                            <div className="mb-3">
                                <p className="text-muted-foreground mb-1 text-xs font-medium uppercase tracking-wide">
                                    Alternatives Considered
                                </p>
                                <ul className="list-inside list-disc space-y-0.5 text-sm">
                                    {alternatives.map((alt, i) => (
                                        <li key={i}>{alt}</li>
                                    ))}
                                </ul>
                            </div>
                        )}
                        {consequences && (
                            <div>
                                <p className="text-muted-foreground mb-1 text-xs font-medium uppercase tracking-wide">Consequences</p>
                                <p className="text-sm">{consequences}</p>
                            </div>
                        )}
                    </div>
                </>
            )}

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
                                <ChunkLink chunkId={conn.targetId}>
                                    {conn.title ?? conn.targetId}
                                </ChunkLink>
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
                                <ChunkLink chunkId={conn.sourceId}>
                                    {conn.title ?? conn.sourceId}
                                </ChunkLink>
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
            <SuggestedConnections chunkId={chunkId} />

            <Separator className="my-6" />
            <RelatedChunks chunkId={chunkId} connections={connections} tags={tags} />

            <Separator className="my-6" />
            <VersionHistory chunkId={chunkId} />
        </div>
    );
}
