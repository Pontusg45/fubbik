import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { AlertTriangle, Check, Clock, Copy, GitCommit } from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

type StaleFlag = {
    id: string;
    chunkId: string;
    reason: string;
    detail: string | null;
    chunkTitle: string;
    chunkType: string;
};

const REASON_CONFIG: Record<string, { icon: typeof GitCommit; label: string }> = {
    file_changed: { icon: GitCommit, label: "File changed" },
    age: { icon: Clock, label: "Outdated" },
    diverged_duplicate: { icon: Copy, label: "Diverged duplicate" },
};

export function AttentionNeeded() {
    const queryClient = useQueryClient();

    const staleQuery = useQuery({
        queryKey: ["stale-flags"],
        queryFn: async () =>
            unwrapEden(
                await api.api.chunks.stale.get({ query: { limit: "10" } })
            ) as StaleFlag[],
    });

    const dismissMutation = useMutation({
        mutationFn: async (flagId: string) =>
            unwrapEden(
                await api.api.chunks({ id: flagId })["dismiss-staleness"].post()
            ),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["stale-flags"] });
            queryClient.invalidateQueries({ queryKey: ["dashboard-health"] });
            toast.success("Flag dismissed");
        },
        onError: () => toast.error("Failed to dismiss flag"),
    });

    const flags = staleQuery.data ?? [];

    if (staleQuery.isLoading || flags.length === 0) return null;

    // Group by reason
    const grouped = flags.reduce<Record<string, StaleFlag[]>>((acc, flag) => {
        const key = flag.reason ?? "unknown";
        if (!acc[key]) acc[key] = [];
        acc[key].push(flag);
        return acc;
    }, {});

    return (
        <div className="mb-6 rounded-lg border border-amber-500/30 bg-amber-500/5">
            <div className="flex items-center gap-2 border-b border-amber-500/20 px-4 py-3">
                <AlertTriangle className="size-4 text-amber-500" />
                <h2 className="text-sm font-semibold">Attention Needed</h2>
                <Badge variant="secondary" size="sm" className="ml-1 bg-amber-500/15 text-amber-600">
                    {flags.length}
                </Badge>
            </div>
            <div className="divide-y divide-amber-500/10 p-2">
                {Object.entries(grouped).map(([reason, items]) => {
                    const config = REASON_CONFIG[reason] ?? {
                        icon: AlertTriangle,
                        label: reason,
                    };
                    const Icon = config.icon;

                    return (
                        <div key={reason} className="py-2 first:pt-0 last:pb-0">
                            <div className="flex items-center gap-1.5 px-2 pb-1.5">
                                <Icon className="size-3 text-amber-500/70" />
                                <span className="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">
                                    {config.label}
                                </span>
                            </div>
                            <div className="space-y-0.5">
                                {items.map((flag) => (
                                    <div
                                        key={flag.id}
                                        className="flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-amber-500/5"
                                    >
                                        <Link
                                            to="/chunks/$chunkId"
                                            params={{ chunkId: flag.chunkId }}
                                            className="min-w-0 flex-1 truncate text-sm hover:underline"
                                        >
                                            {flag.chunkTitle}
                                        </Link>
                                        {flag.detail && (
                                            <span className="text-muted-foreground hidden shrink-0 text-[11px] sm:inline">
                                                {flag.detail}
                                            </span>
                                        )}
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            className="size-6 shrink-0 p-0"
                                            onClick={() => dismissMutation.mutate(flag.id)}
                                            disabled={dismissMutation.isPending}
                                        >
                                            <Check className="size-3.5" />
                                        </Button>
                                    </div>
                                ))}
                            </div>
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
