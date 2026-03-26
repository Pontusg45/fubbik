import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Check, ChevronDown, ChevronRight, Inbox, Pencil, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { PageEmpty, PageLoading } from "@/components/ui/page";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

const TYPE_COLORS: Record<string, string> = {
    note: "bg-blue-500/10 text-blue-600 border-blue-500/30 dark:text-blue-400",
    document: "bg-purple-500/10 text-purple-600 border-purple-500/30 dark:text-purple-400",
    reference: "bg-amber-500/10 text-amber-600 border-amber-500/30 dark:text-amber-400",
    schema: "bg-emerald-500/10 text-emerald-600 border-emerald-500/30 dark:text-emerald-400",
    checklist: "bg-rose-500/10 text-rose-600 border-rose-500/30 dark:text-rose-400"
};

interface Chunk {
    id: string;
    title: string;
    type: string;
    content: string;
    reviewStatus: string | null;
    origin: string | null;
    createdAt: Date;
}

export function ReviewQueueContent() {
    const queryClient = useQueryClient();
    const { codebaseId } = useActiveCodebase();
    const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
    const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());

    const { data, isLoading } = useQuery({
        queryKey: ["review-queue", codebaseId],
        queryFn: async () => {
            try {
                return unwrapEden(
                    await api.api.chunks.get({
                        query: {
                            origin: "ai",
                            reviewStatus: "draft",
                            limit: "50",
                            sort: "newest",
                            ...(codebaseId === "global" ? { global: "true" } : codebaseId ? { codebaseId } : {})
                        }
                    })
                );
            } catch {
                return null;
            }
        }
    });

    const approveMutation = useMutation({
        mutationFn: async (ids: string[]) => {
            await Promise.all(
                ids.map(async id => {
                    const { error } = await api.api.chunks({ id }).patch({ reviewStatus: "approved" });
                    if (error) throw new Error(`Failed to approve chunk ${id}`);
                })
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["review-queue"] });
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            setSelectedIds(new Set());
            toast.success("Chunks approved");
        },
        onError: () => {
            toast.error("Failed to approve chunks");
        }
    });

    const rejectMutation = useMutation({
        mutationFn: async (ids: string[]) => {
            await Promise.all(
                ids.map(async id => {
                    const { error } = await (api.api.chunks as any)[id].archive.post();
                    if (error) throw new Error(`Failed to reject chunk ${id}`);
                })
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["review-queue"] });
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            setSelectedIds(new Set());
            toast.success("Chunks rejected and archived");
        },
        onError: () => {
            toast.error("Failed to reject chunks");
        }
    });

    const chunks: Chunk[] = (data?.chunks as unknown as Chunk[]) ?? [];
    const allSelected = chunks.length > 0 && selectedIds.size === chunks.length;
    const someSelected = selectedIds.size > 0;

    function toggleSelect(id: string) {
        setSelectedIds(prev => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return next;
        });
    }

    function toggleAll() {
        if (allSelected) {
            setSelectedIds(new Set());
        } else {
            setSelectedIds(new Set(chunks.map(c => c.id)));
        }
    }

    function toggleExpanded(id: string) {
        setExpandedIds(prev => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return next;
        });
    }

    const isBusy = approveMutation.isPending || rejectMutation.isPending;

    if (isLoading) {
        return <PageLoading count={5} />;
    }

    if (chunks.length === 0) {
        return (
            <PageEmpty
                icon={Inbox}
                title="All caught up!"
                description="No AI-generated drafts to review."
            />
        );
    }

    return (
        <>
            {/* Bulk action bar */}
            <div className="mb-4 flex items-center gap-3">
                <Checkbox checked={allSelected} onCheckedChange={toggleAll} />
                <span className="text-muted-foreground text-sm">
                    {someSelected ? `${selectedIds.size} selected` : `${chunks.length} drafts`}
                </span>
                {someSelected && (
                    <div className="ml-auto flex items-center gap-2">
                        <Button
                            size="sm"
                            variant="outline"
                            className="text-emerald-600 hover:bg-emerald-50 dark:text-emerald-400 dark:hover:bg-emerald-500/10"
                            disabled={isBusy}
                            onClick={() => approveMutation.mutate([...selectedIds])}
                        >
                            <Check className="mr-1 size-3.5" />
                            Approve ({selectedIds.size})
                        </Button>
                        <Button
                            size="sm"
                            variant="outline"
                            className="text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-500/10"
                            disabled={isBusy}
                            onClick={() => rejectMutation.mutate([...selectedIds])}
                        >
                            <X className="mr-1 size-3.5" />
                            Reject ({selectedIds.size})
                        </Button>
                    </div>
                )}
            </div>

            {/* Chunk list */}
            <div className="space-y-2">
                {chunks.map(chunk => {
                    const isExpanded = expandedIds.has(chunk.id);
                    const isSelected = selectedIds.has(chunk.id);
                    return (
                        <div
                            key={chunk.id}
                            className={`rounded-lg border p-3 transition-colors ${isSelected ? "border-primary/40 bg-primary/5" : ""}`}
                        >
                            <div className="flex items-center gap-3">
                                <Checkbox checked={isSelected} onCheckedChange={() => toggleSelect(chunk.id)} />
                                <button
                                    onClick={() => toggleExpanded(chunk.id)}
                                    className="text-muted-foreground hover:text-foreground shrink-0 transition-colors"
                                >
                                    {isExpanded ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                                </button>
                                <div className="min-w-0 flex-1">
                                    <div className="flex items-center gap-2">
                                        <Link
                                            to="/chunks/$chunkId"
                                            params={{ chunkId: chunk.id }}
                                            className="hover:underline truncate text-sm font-medium"
                                        >
                                            {chunk.title}
                                        </Link>
                                        <Badge variant="outline" className={TYPE_COLORS[chunk.type] ?? ""}>
                                            {chunk.type}
                                        </Badge>
                                        <Badge variant="secondary" className="text-xs">
                                            Draft
                                        </Badge>
                                    </div>
                                </div>
                                <div className="flex shrink-0 items-center gap-1">
                                    <Button
                                        size="sm"
                                        variant="ghost"
                                        className="text-emerald-600 hover:bg-emerald-50 dark:text-emerald-400 dark:hover:bg-emerald-500/10"
                                        disabled={isBusy}
                                        onClick={() => approveMutation.mutate([chunk.id])}
                                        title="Approve"
                                    >
                                        <Check className="size-4" />
                                    </Button>
                                    <Button
                                        size="sm"
                                        variant="ghost"
                                        render={<Link to="/chunks/$chunkId/edit" params={{ chunkId: chunk.id }} />}
                                        title="Edit"
                                    >
                                        <Pencil className="size-4" />
                                    </Button>
                                    <Button
                                        size="sm"
                                        variant="ghost"
                                        className="text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-500/10"
                                        disabled={isBusy}
                                        onClick={() => rejectMutation.mutate([chunk.id])}
                                        title="Reject"
                                    >
                                        <X className="size-4" />
                                    </Button>
                                </div>
                            </div>
                            {isExpanded && (
                                <div className="text-muted-foreground mt-2 ml-10 text-sm whitespace-pre-wrap line-clamp-6">
                                    {chunk.content}
                                </div>
                            )}
                        </div>
                    );
                })}
            </div>
        </>
    );
}
