import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Clock, Copy, GitCommit, X } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

type StaleFlag = {
    id: string;
    chunkId: string;
    reason: string;
    detail: string | null;
    relatedChunkId: string | null;
    chunkTitle: string;
    chunkType: string;
    detectedAt: Date;
};

const REASON_ICON: Record<string, typeof GitCommit> = {
    file_changed: GitCommit,
    age: Clock,
    diverged_duplicate: Copy,
};

function getMessage(reason: string, detail: string | null): string {
    switch (reason) {
        case "file_changed":
            return detail
                ? `A referenced file has changed: ${detail}`
                : "A referenced file has changed since this chunk was last updated.";
        case "age":
            return detail ?? "This chunk has not been updated in a long time and may be outdated.";
        case "diverged_duplicate":
            return detail
                ? `This chunk may have diverged from a similar chunk: ${detail}`
                : "This chunk may have diverged from a similar chunk.";
        default:
            return detail ?? "This chunk may need attention.";
    }
}

export function StalenessBanner({ chunkId }: { chunkId: string }) {
    const queryClient = useQueryClient();

    const staleQuery = useQuery({
        queryKey: ["stale-flags"],
        queryFn: async () =>
            unwrapEden(
                await api.api.chunks.stale.get({ query: {} })
            ) as StaleFlag[],
        staleTime: 60_000,
    });

    const dismissMutation = useMutation({
        mutationFn: async (flagId: string) =>
            unwrapEden(
                await api.api.chunks({ id: flagId })["dismiss-staleness"].post()
            ),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["stale-flags"] });
            queryClient.invalidateQueries({ queryKey: ["stale-count"] });
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            toast.success("Staleness flag dismissed");
        },
        onError: () => toast.error("Failed to dismiss flag"),
    });

    const flags = (staleQuery.data ?? []).filter(f => f.chunkId === chunkId);

    if (flags.length === 0) return null;

    return (
        <div className="mb-6 space-y-2">
            {flags.map(flag => {
                const Icon = REASON_ICON[flag.reason] ?? Clock;
                return (
                    <div
                        key={flag.id}
                        className="flex items-start gap-3 rounded-lg border border-amber-500/30 bg-amber-500/5 px-4 py-3"
                    >
                        <Icon className="mt-0.5 size-4 shrink-0 text-amber-500" />
                        <div className="min-w-0 flex-1">
                            <p className="text-sm text-amber-800 dark:text-amber-300">
                                {getMessage(flag.reason, flag.detail)}
                            </p>
                            {flag.reason === "diverged_duplicate" && flag.relatedChunkId && (
                                <Link
                                    to="/compare"
                                    search={{ left: chunkId, right: flag.relatedChunkId }}
                                    className="mt-1 inline-block text-xs text-amber-600 underline hover:text-amber-700 dark:text-amber-400 dark:hover:text-amber-300"
                                >
                                    Compare side by side
                                </Link>
                            )}
                        </div>
                        <Button
                            variant="ghost"
                            size="sm"
                            className="size-6 shrink-0 p-0 text-amber-500 hover:text-amber-700"
                            onClick={() => dismissMutation.mutate(flag.id)}
                            disabled={dismissMutation.isPending}
                            title="Dismiss"
                        >
                            <X className="size-3.5" />
                        </Button>
                    </div>
                );
            })}
        </div>
    );
}
