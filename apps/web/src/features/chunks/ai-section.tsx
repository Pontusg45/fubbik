import { useMutation } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Bot, Loader2, Network, Sparkles } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { api } from "@/utils/api";

export function AiSection({ chunkId }: { chunkId: string }) {
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
