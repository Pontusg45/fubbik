import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ChevronDown, ChevronRight, Lightbulb, Loader2, Plus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { api } from "@/utils/api";

interface Suggestion {
    id: string;
    title: string;
    type: string;
    reason: string;
}

export function SuggestedConnections({ chunkId }: { chunkId: string }) {
    const [expanded, setExpanded] = useState(false);
    const queryClient = useQueryClient();

    const suggestionsQuery = useQuery({
        queryKey: ["chunk-suggestions", chunkId],
        queryFn: async () => {
            const { data, error } = await api.api.chunks({ id: chunkId }).suggestions.get();
            if (error) throw new Error("Failed to load suggestions");
            return data as Suggestion[];
        },
        enabled: expanded
    });

    const connectMutation = useMutation({
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
            queryClient.invalidateQueries({ queryKey: ["chunk-suggestions", chunkId] });
            toast.success("Connection created");
        },
        onError: () => {
            toast.error("Failed to create connection");
        }
    });

    const suggestions = suggestionsQuery.data ?? [];

    return (
        <Card>
            <CardHeader>
                <button
                    onClick={() => setExpanded(!expanded)}
                    className="flex w-full items-center gap-2"
                >
                    <CardTitle className="flex items-center gap-2 text-sm">
                        <Lightbulb className="size-4" />
                        Suggested Connections
                    </CardTitle>
                    {expanded ? (
                        <ChevronDown className="text-muted-foreground ml-auto size-4" />
                    ) : (
                        <ChevronRight className="text-muted-foreground ml-auto size-4" />
                    )}
                </button>
            </CardHeader>

            {expanded && (
                <CardPanel className="space-y-2 pt-0">
                    {suggestionsQuery.isLoading && (
                        <div className="flex items-center gap-2 py-2">
                            <Loader2 className="text-muted-foreground size-4 animate-spin" />
                            <span className="text-muted-foreground text-sm">Finding suggestions...</span>
                        </div>
                    )}

                    {suggestionsQuery.isError && (
                        <p className="text-muted-foreground text-sm">Failed to load suggestions.</p>
                    )}

                    {!suggestionsQuery.isLoading && suggestions.length === 0 && !suggestionsQuery.isError && (
                        <p className="text-muted-foreground text-sm">No suggestions found.</p>
                    )}

                    {suggestions.map(suggestion => (
                        <div
                            key={suggestion.id}
                            className="hover:bg-muted flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                        >
                            <div className="flex-1">
                                <Link
                                    to="/chunks/$chunkId"
                                    params={{ chunkId: suggestion.id }}
                                    className="font-medium"
                                >
                                    {suggestion.title}
                                </Link>
                                <div className="mt-0.5 flex items-center gap-2">
                                    <Badge variant="secondary" size="sm" className="text-[10px]">
                                        {suggestion.type}
                                    </Badge>
                                    <span className="text-muted-foreground text-xs">{suggestion.reason}</span>
                                </div>
                            </div>
                            <Button
                                variant="outline"
                                size="sm"
                                onClick={() => connectMutation.mutate(suggestion.id)}
                                disabled={connectMutation.isPending}
                                className="ml-2"
                            >
                                <Plus className="mr-1 size-3" />
                                Connect
                            </Button>
                        </div>
                    ))}
                </CardPanel>
            )}
        </Card>
    );
}
