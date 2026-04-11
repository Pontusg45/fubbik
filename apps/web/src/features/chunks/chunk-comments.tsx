import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { MessageSquare, Pencil, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { api } from "@/utils/api";

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

export function ChunkComments({ chunkId }: { chunkId: string }) {
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
