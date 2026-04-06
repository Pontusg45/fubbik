import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Link2, Loader2, Sparkles, X } from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useLocalStorage } from "@/hooks/use-local-storage";
import { api } from "@/utils/api";

interface RelatedSuggestionsProps {
    chunkId: string;
    chunkTitle: string;
    connectedIds: string[];
}

export function RelatedSuggestions({ chunkId, chunkTitle, connectedIds }: RelatedSuggestionsProps) {
    const queryClient = useQueryClient();
    const [dismissedIds, setDismissedIds] = useLocalStorage<string[]>(
        `fubbik:dismissed-suggestions:${chunkId}`,
        []
    );

    const { data, isLoading, isError } = useQuery({
        queryKey: ["related-suggestions", chunkId],
        queryFn: async () => {
            const { data, error } = await api.api.chunks.search.semantic.get({
                query: { q: chunkTitle, limit: "8" }
            });
            if (error) throw new Error("Failed to load similar chunks");
            return data as Array<{
                id: string;
                title: string;
                type: string;
                similarity: number;
            }>;
        }
    });

    const linkMutation = useMutation({
        mutationFn: async (targetId: string) => {
            const { error } = await api.api.connections.post({
                sourceId: chunkId,
                targetId,
                relation: "related_to"
            });
            if (error) throw new Error("Failed to create connection");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            queryClient.invalidateQueries({ queryKey: ["related-suggestions", chunkId] });
            toast.success("Connection created");
        },
        onError: () => {
            toast.error("Failed to create connection");
        }
    });

    const connectedSet = new Set(connectedIds);
    const dismissedSet = new Set(dismissedIds);

    const suggestions = (data ?? [])
        .filter(
            item =>
                item.id !== chunkId &&
                !connectedSet.has(item.id) &&
                !dismissedSet.has(item.id)
        )
        .slice(0, 5);

    if (isLoading) {
        return (
            <div className="mt-6 rounded-lg border-2 border-dashed p-4">
                <div className="flex items-center gap-2">
                    <Loader2 className="text-muted-foreground size-4 animate-spin" />
                    <span className="text-muted-foreground text-sm">Finding related chunks...</span>
                </div>
            </div>
        );
    }

    if (isError || suggestions.length === 0) {
        return null;
    }

    return (
        <div className="mt-6 rounded-lg border-2 border-dashed p-4">
            <h3 className="mb-3 flex items-center gap-2 text-sm font-medium">
                <Sparkles className="size-4" />
                You might want to connect
            </h3>
            <div className="space-y-2">
                {suggestions.map(item => (
                    <div
                        key={item.id}
                        className="hover:bg-muted flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                    >
                        <div className="flex-1 min-w-0">
                            <Link
                                to="/chunks/$chunkId"
                                params={{ chunkId: item.id }}
                                className="font-medium hover:underline truncate block"
                            >
                                {item.title}
                            </Link>
                            <div className="mt-0.5 flex items-center gap-2">
                                <Badge variant="secondary" size="sm" className="text-[10px]">
                                    {item.type}
                                </Badge>
                                <span className="text-muted-foreground text-xs">
                                    {Math.round(item.similarity * 100)}% similar
                                </span>
                            </div>
                        </div>
                        <div className="ml-2 flex shrink-0 items-center gap-1">
                            <Button
                                variant="outline"
                                size="sm"
                                onClick={() => linkMutation.mutate(item.id)}
                                disabled={linkMutation.isPending}
                                title="Link"
                            >
                                <Link2 className="size-3" />
                            </Button>
                            <Button
                                variant="ghost"
                                size="sm"
                                onClick={() =>
                                    setDismissedIds(prev => [...prev, item.id])
                                }
                                title="Dismiss"
                            >
                                <X className="size-3" />
                            </Button>
                        </div>
                    </div>
                ))}
            </div>
        </div>
    );
}
