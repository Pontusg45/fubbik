import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link2Off, Plus, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useApiQuery } from "@/hooks/use-api-query";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface CellPanelProps {
    matrixId: string;
    cellId: string;
    ruleTitle: string;
    dimensionName: string;
    onClose: () => void;
}

interface CellRequirement {
    id: string;
    title: string;
    status: string;
}

const STATUS_BADGE: Record<string, "success" | "destructive" | "secondary"> = {
    passing: "success",
    failing: "destructive",
    untested: "secondary",
};

export function CellPanel({ matrixId, cellId, ruleTitle, dimensionName, onClose }: CellPanelProps) {
    const queryClient = useQueryClient();
    const [requirementId, setRequirementId] = useState("");

    const requirementsQuery = useApiQuery<CellRequirement[]>({
        queryKey: ["matrix-cell-requirements", cellId],
        queryFn: () =>
            api.api.matrices({ id: matrixId }).cells({ cellId }).requirements.get(),
        fallback: [],
    });

    const linkMutation = useMutation({
        mutationFn: async (reqId: string) =>
            unwrapEden(
                await api.api.matrices({ id: matrixId }).cells({ cellId }).requirements.post({ requirementId: reqId })
            ),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-cell-requirements", cellId] });
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            setRequirementId("");
            toast.success("Requirement linked");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to link requirement";
            toast.error(msg);
        },
    });

    const unlinkMutation = useMutation({
        mutationFn: async (reqId: string) =>
            unwrapEden(
                await api.api.matrices({ id: matrixId }).cells({ cellId }).requirements({ reqId }).delete()
            ),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-cell-requirements", cellId] });
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            toast.success("Requirement unlinked");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to unlink requirement";
            toast.error(msg);
        },
    });

    function handleLink(e: React.FormEvent) {
        e.preventDefault();
        const id = requirementId.trim();
        if (!id) return;
        linkMutation.mutate(id);
    }

    const requirements = Array.isArray(requirementsQuery.data) ? requirementsQuery.data : [];

    return (
        <div className="fixed inset-y-0 right-0 z-40 flex w-full max-w-sm flex-col border-l bg-background shadow-lg">
            {/* Header */}
            <div className="flex items-start justify-between gap-3 border-b px-4 py-4">
                <div className="min-w-0">
                    <h3 className="font-semibold leading-tight">{ruleTitle}</h3>
                    <p className="text-muted-foreground mt-0.5 text-sm">{dimensionName}</p>
                </div>
                <Button
                    size="icon-xs"
                    variant="ghost"
                    onClick={onClose}
                    aria-label="Close panel"
                >
                    <X className="size-4" />
                </Button>
            </div>

            {/* Requirements list */}
            <div className="flex-1 overflow-y-auto px-4 py-3">
                <h4 className="text-muted-foreground mb-2 text-xs font-semibold uppercase tracking-wide">
                    Requirements ({requirements.length})
                </h4>
                {requirements.length === 0 ? (
                    <p className="text-muted-foreground py-4 text-center text-sm">
                        No requirements linked to this cell.
                    </p>
                ) : (
                    <ul className="space-y-2">
                        {requirements.map(req => (
                            <li
                                key={req.id}
                                className="flex items-center gap-2 rounded-md border px-3 py-2"
                            >
                                <div className="min-w-0 flex-1">
                                    <span className="text-sm font-medium line-clamp-1">{req.title}</span>
                                </div>
                                <Badge
                                    variant={STATUS_BADGE[req.status] ?? "secondary"}
                                    size="sm"
                                >
                                    {req.status}
                                </Badge>
                                <Button
                                    size="icon-xs"
                                    variant="ghost"
                                    onClick={() => unlinkMutation.mutate(req.id)}
                                    disabled={unlinkMutation.isPending}
                                    title="Unlink requirement"
                                    aria-label={`Unlink ${req.title}`}
                                >
                                    <Link2Off className="size-3.5" />
                                </Button>
                            </li>
                        ))}
                    </ul>
                )}
            </div>

            {/* Link input */}
            <div className="border-t px-4 py-3">
                <form onSubmit={handleLink} className="flex gap-2">
                    <Input
                        placeholder="Requirement ID..."
                        value={requirementId}
                        onChange={e => setRequirementId(e.target.value)}
                        size="sm"
                    />
                    <Button
                        type="submit"
                        size="sm"
                        disabled={!requirementId.trim() || linkMutation.isPending}
                    >
                        <Plus className="size-3.5" />
                        Link
                    </Button>
                </form>
            </div>
        </div>
    );
}
