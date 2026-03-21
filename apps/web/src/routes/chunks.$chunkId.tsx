import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { Archive, ArrowLeft, Bot, Calendar, Clock, Code, Download, Edit, FileCode, FileText, Hash, MessageSquare, Network, Pencil, Scale, Star, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { MarkdownRenderer } from "@/components/markdown-renderer";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { AiSection } from "@/features/chunks/ai-section";
import { ChunkHealthBadge } from "@/features/chunks/chunk-health-badge";
import { ChunkLink } from "@/features/chunks/chunk-link";
import { getChunkSize } from "@/features/chunks/chunk-size";
import { DeleteConnectionButton } from "@/features/chunks/delete-connection-button";
import { InlineTagEditor } from "@/features/chunks/inline-tag-editor";
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
    const [showDeleteDialog, setShowDeleteDialog] = useState(false);

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

    const archiveMutation = useMutation({
        mutationFn: async () => {
            const { error } = await (api.api.chunks as any)[chunkId].archive.post();
            if (error) throw new Error("Failed to archive chunk");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            toast.success("Chunk archived");
            navigate({ to: "/chunks", search: {} });
        },
        onError: () => {
            toast.error("Failed to archive chunk");
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

    const tagMutation = useMutation({
        mutationFn: async (tags: string[]) => {
            const { error } = await api.api.chunks({ id: chunkId }).patch({ tags });
            if (error) throw new Error("Failed to update tags");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            toast.success("Tags updated");
        },
        onError: () => {
            toast.error("Failed to update tags");
        }
    });

    if (isLoading) {
        return (
            <div className="container mx-auto max-w-3xl px-4 py-8">
                <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="mt-8 space-y-6">
                    {/* Title skeleton */}
                    <div className="space-y-3">
                        <Skeleton className="h-5 w-20" />
                        <Skeleton className="h-8 w-2/3" />
                        <div className="flex gap-4">
                            <Skeleton className="h-3 w-32" />
                            <Skeleton className="h-3 w-32" />
                        </div>
                    </div>
                    <Separator />
                    {/* Content skeleton */}
                    <div className="space-y-3">
                        <Skeleton className="h-4 w-full" />
                        <Skeleton className="h-4 w-full" />
                        <Skeleton className="h-4 w-5/6" />
                        <Skeleton className="h-4 w-full" />
                        <Skeleton className="h-4 w-3/4" />
                        <Skeleton className="h-4 w-full" />
                        <Skeleton className="h-4 w-2/3" />
                    </div>
                    <Separator />
                    {/* Metadata skeleton */}
                    <div className="space-y-2">
                        <Skeleton className="h-5 w-32" />
                        <Skeleton className="h-12 w-full" />
                    </div>
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
    const currentCodebases = (data as Record<string, unknown>).codebases as
        | Array<{ id: string; name: string }>
        | undefined;
    const currentCodebaseNames = new Set(currentCodebases?.map(c => c.name) ?? []);
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
    const healthScore = (data as Record<string, unknown>).healthScore as
        | { total: number; breakdown: { freshness: number; completeness: number; richness: number; connectivity: number }; issues: string[] }
        | undefined;
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
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                            const md = `# ${chunk.title}\n\n**Type:** ${chunk.type}\n**Tags:** ${tags.map((t: any) => t.name || t).join(", ")}\n\n${chunk.content}`;
                            const blob = new Blob([md], { type: "text/markdown" });
                            const url = URL.createObjectURL(blob);
                            const a = document.createElement("a");
                            a.href = url;
                            a.download = `${chunk.title.replace(/[^a-z0-9]/gi, "-").toLowerCase()}.md`;
                            a.click();
                            URL.revokeObjectURL(url);
                        }}
                    >
                        <Download className="size-3.5" />
                        Export MD
                    </Button>
                    <Button variant="outline" size="sm" render={<Link to="/chunks/$chunkId/edit" params={{ chunkId }} />}>
                        <Edit className="size-3.5" />
                        Edit
                    </Button>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => archiveMutation.mutate()}
                        disabled={archiveMutation.isPending}
                    >
                        <Archive className="size-3.5" />
                        {archiveMutation.isPending ? "Archiving..." : "Archive"}
                    </Button>
                    <Button
                        variant="outline"
                        size="sm"
                        className="text-destructive"
                        onClick={() => setShowDeleteDialog(true)}
                        disabled={deleteMutation.isPending}
                    >
                        <Trash2 className="size-3.5" />
                        {deleteMutation.isPending ? "Deleting..." : "Delete"}
                    </Button>
                    <ConfirmDialog
                        open={showDeleteDialog}
                        onOpenChange={setShowDeleteDialog}
                        title="Delete chunk"
                        description="Permanently delete this chunk? This cannot be undone."
                        confirmLabel="Delete"
                        confirmVariant="destructive"
                        onConfirm={() => {
                            setShowDeleteDialog(false);
                            deleteMutation.mutate();
                        }}
                        loading={deleteMutation.isPending}
                    />
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
                    {healthScore && <ChunkHealthBadge healthScore={healthScore} />}
                </div>
            </div>

            <div className="mb-2">
                <InlineTagEditor
                    tags={tags}
                    onUpdate={(newTags) => tagMutation.mutate(newTags)}
                    loading={tagMutation.isPending}
                />
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
            <ChunkComments chunkId={chunkId} />

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
                                    {conn.codebaseName && !currentCodebaseNames.has(conn.codebaseName) && (
                                        <Badge variant="outline" size="sm" className="text-[10px]">
                                            {conn.codebaseName}
                                        </Badge>
                                    )}
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
                                    {conn.codebaseName && !currentCodebaseNames.has(conn.codebaseName) && (
                                        <Badge variant="outline" size="sm" className="text-[10px]">
                                            {conn.codebaseName}
                                        </Badge>
                                    )}
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

function timeAgo(date: string | Date): string {
    const seconds = Math.floor((Date.now() - new Date(date).getTime()) / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}

function ChunkComments({ chunkId }: { chunkId: string }) {
    const queryClient = useQueryClient();
    const [expanded, setExpanded] = useState(false);
    const [newComment, setNewComment] = useState("");
    const [editingId, setEditingId] = useState<string | null>(null);
    const [editContent, setEditContent] = useState("");

    const commentsQuery = useQuery({
        queryKey: ["comments", chunkId],
        queryFn: async () => {
            const { data, error } = await (api.api.chunks as any)({ id: chunkId }).comments.get();
            if (error) throw new Error("Failed to load comments");
            return data as Array<{
                id: string;
                chunkId: string;
                userId: string;
                content: string;
                createdAt: string;
                updatedAt: string;
            }>;
        }
    });

    const addMutation = useMutation({
        mutationFn: async (content: string) => {
            const { error } = await (api.api.chunks as any)({ id: chunkId }).comments.post({ content });
            if (error) throw new Error("Failed to add comment");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["comments", chunkId] });
            setNewComment("");
            toast.success("Comment added");
        },
        onError: () => toast.error("Failed to add comment")
    });

    const updateMutation = useMutation({
        mutationFn: async ({ id, content }: { id: string; content: string }) => {
            const { error } = await (api.api as any).comments({ id }).patch({ content });
            if (error) throw new Error("Failed to update comment");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["comments", chunkId] });
            setEditingId(null);
            toast.success("Comment updated");
        },
        onError: () => toast.error("Failed to update comment")
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => {
            const { error } = await (api.api as any).comments({ id }).delete();
            if (error) throw new Error("Failed to delete comment");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["comments", chunkId] });
            toast.success("Comment deleted");
        },
        onError: () => toast.error("Failed to delete comment")
    });

    const comments = commentsQuery.data ?? [];
    const count = comments.length;

    return (
        <Card>
            <CardHeader>
                <button
                    className="flex w-full items-center justify-between text-left"
                    onClick={() => setExpanded(!expanded)}
                >
                    <CardTitle className="flex items-center gap-2 text-sm">
                        <MessageSquare className="size-4" />
                        Comments
                        <Badge variant="secondary" size="sm">
                            {count}
                        </Badge>
                    </CardTitle>
                    <span className="text-muted-foreground text-xs">{expanded ? "Hide" : "Show"}</span>
                </button>
            </CardHeader>
            {expanded && (
                <CardPanel className="space-y-3 pt-0">
                    {comments.map(comment => (
                        <div key={comment.id} className="rounded-md border px-3 py-2">
                            {editingId === comment.id ? (
                                <div className="space-y-2">
                                    <textarea
                                        className="border-input bg-background w-full rounded-md border px-3 py-2 text-sm"
                                        rows={3}
                                        value={editContent}
                                        onChange={e => setEditContent(e.target.value)}
                                    />
                                    <div className="flex gap-2">
                                        <Button
                                            size="sm"
                                            onClick={() => updateMutation.mutate({ id: comment.id, content: editContent })}
                                            disabled={updateMutation.isPending || !editContent.trim()}
                                        >
                                            Save
                                        </Button>
                                        <Button size="sm" variant="ghost" onClick={() => setEditingId(null)}>
                                            Cancel
                                        </Button>
                                    </div>
                                </div>
                            ) : (
                                <>
                                    <div className="flex items-start justify-between">
                                        <p className="text-sm whitespace-pre-wrap">{comment.content}</p>
                                        <div className="flex shrink-0 items-center gap-1 ml-2">
                                            <button
                                                className="text-muted-foreground hover:text-foreground p-0.5"
                                                onClick={() => {
                                                    setEditingId(comment.id);
                                                    setEditContent(comment.content);
                                                }}
                                                title="Edit"
                                            >
                                                <Pencil className="size-3" />
                                            </button>
                                            <button
                                                className="text-muted-foreground hover:text-destructive p-0.5"
                                                onClick={() => deleteMutation.mutate(comment.id)}
                                                title="Delete"
                                            >
                                                <Trash2 className="size-3" />
                                            </button>
                                        </div>
                                    </div>
                                    <p className="text-muted-foreground mt-1 text-xs">
                                        {timeAgo(comment.createdAt)}
                                        {comment.updatedAt !== comment.createdAt && " (edited)"}
                                    </p>
                                </>
                            )}
                        </div>
                    ))}
                    {count === 0 && (
                        <p className="text-muted-foreground text-sm">No comments yet</p>
                    )}
                    <div className="space-y-2 pt-2">
                        <textarea
                            className="border-input bg-background w-full rounded-md border px-3 py-2 text-sm"
                            rows={2}
                            placeholder="Add a comment..."
                            value={newComment}
                            onChange={e => setNewComment(e.target.value)}
                        />
                        <Button
                            size="sm"
                            onClick={() => addMutation.mutate(newComment)}
                            disabled={addMutation.isPending || !newComment.trim()}
                        >
                            {addMutation.isPending ? "Adding..." : "Add Comment"}
                        </Button>
                    </div>
                </CardPanel>
            )}
        </Card>
    );
}
