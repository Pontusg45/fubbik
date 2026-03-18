import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Check, ChevronDown, ChevronRight, Plus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface AssumptionResolverProps {
    assumption: { id: string; description: string; resolved: boolean; resolution: string | null };
    sessionId: string;
}

export function AssumptionResolver({ assumption, sessionId }: AssumptionResolverProps) {
    const queryClient = useQueryClient();
    const [expanded, setExpanded] = useState(false);
    const [resolution, setResolution] = useState("");

    const resolveMutation = useMutation({
        mutationFn: async () => {
            return unwrapEden(
                await (api.api.sessions as any)({ id: sessionId }).assumptions({ assumptionId: assumption.id }).patch({
                    resolved: true,
                    resolution: resolution.trim() || undefined
                })
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["session", sessionId] });
            setExpanded(false);
            setResolution("");
            toast.success("Assumption resolved");
        },
        onError: () => toast.error("Failed to resolve assumption")
    });

    if (assumption.resolved) {
        return (
            <div className="flex items-start gap-3 rounded-lg border border-emerald-500/30 bg-emerald-500/5 px-4 py-3">
                <Check className="mt-0.5 size-4 shrink-0 text-emerald-600 dark:text-emerald-400" />
                <div className="min-w-0 flex-1">
                    <p className="text-sm">{assumption.description}</p>
                    {assumption.resolution && (
                        <p className="text-muted-foreground mt-1 text-xs">{assumption.resolution}</p>
                    )}
                </div>
                <Badge variant="outline" className="border-emerald-500/30 text-emerald-600 dark:text-emerald-400">
                    Resolved
                </Badge>
            </div>
        );
    }

    return (
        <div className="rounded-lg border border-amber-500/30 bg-amber-500/5 px-4 py-3">
            <div className="flex items-start gap-3">
                <div className="mt-0.5 size-2 shrink-0 rounded-full bg-amber-500" />
                <div className="min-w-0 flex-1">
                    <p className="text-sm">{assumption.description}</p>
                </div>
                <div className="flex items-center gap-2">
                    <Link
                        to="/chunks/new"
                        className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1 text-xs transition-colors"
                    >
                        <Plus className="size-3" />
                        Create chunk
                    </Link>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => setExpanded(!expanded)}
                    >
                        {expanded ? <ChevronDown className="mr-1 size-3" /> : <ChevronRight className="mr-1 size-3" />}
                        Resolve
                    </Button>
                </div>
            </div>
            {expanded && (
                <div className="mt-3 flex items-center gap-2 pl-5">
                    <Input
                        value={resolution}
                        onChange={e => setResolution(e.target.value)}
                        placeholder="Resolution notes..."
                        className="flex-1"
                    />
                    <Button
                        size="sm"
                        onClick={() => resolveMutation.mutate()}
                        disabled={resolveMutation.isPending}
                    >
                        {resolveMutation.isPending ? "Saving..." : "Save"}
                    </Button>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                            setExpanded(false);
                            setResolution("");
                        }}
                    >
                        Cancel
                    </Button>
                </div>
            )}
        </div>
    );
}
