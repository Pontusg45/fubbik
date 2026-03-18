import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Plus, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface DependencySectionProps {
    requirementId: string;
}

const STATUS_STYLES: Record<string, string> = {
    passing: "bg-emerald-500/10 text-emerald-600 border-emerald-500/30 dark:text-emerald-400",
    failing: "bg-red-500/10 text-red-600 border-red-500/30 dark:text-red-400",
    untested: "bg-muted text-muted-foreground"
};

interface DepItem {
    id: string;
    title: string;
    status: string;
    priority: string | null;
}

export function DependencySection({ requirementId }: DependencySectionProps) {
    const queryClient = useQueryClient();
    const [showAdd, setShowAdd] = useState(false);
    const [search, setSearch] = useState("");

    const { data } = useQuery({
        queryKey: ["dependencies", requirementId],
        queryFn: async () => {
            return unwrapEden(
                await api.api.requirements({ id: requirementId }).dependencies.get()
            ) as { dependsOn: DepItem[]; dependedOnBy: DepItem[] };
        }
    });

    const searchQuery = useQuery({
        queryKey: ["requirements-search", search],
        queryFn: async () => {
            const result = unwrapEden(
                await api.api.requirements.get({ query: { search } })
            ) as { requirements: Array<{ id: string; title: string; status: string }>; total: number };
            return result.requirements;
        },
        enabled: showAdd && search.length > 0
    });

    const addMutation = useMutation({
        mutationFn: async (dependsOnId: string) => {
            return unwrapEden(
                await api.api.requirements({ id: requirementId }).dependencies.post({ dependsOnId })
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["dependencies", requirementId] });
            toast.success("Dependency added");
            setSearch("");
        },
        onError: () => toast.error("Failed to add dependency")
    });

    const removeMutation = useMutation({
        mutationFn: async (dependsOnId: string) => {
            return unwrapEden(
                await api.api.requirements({ id: requirementId }).dependencies({ dependsOnId }).delete()
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["dependencies", requirementId] });
            toast.success("Dependency removed");
        },
        onError: () => toast.error("Failed to remove dependency")
    });

    const dependsOn = data?.dependsOn ?? [];
    const dependedOnBy = data?.dependedOnBy ?? [];

    const existingIds = new Set([
        requirementId,
        ...dependsOn.map(d => d.id),
        ...dependedOnBy.map(d => d.id)
    ]);

    const searchResults = (searchQuery.data ?? []).filter(r => !existingIds.has(r.id));

    return (
        <div>
            <div className="mb-3 flex items-center justify-between">
                <h2 className="text-sm font-semibold">Dependencies</h2>
                <Button variant="outline" size="sm" onClick={() => setShowAdd(!showAdd)}>
                    <Plus className="mr-1 size-3.5" />
                    Add dependency
                </Button>
            </div>

            {showAdd && (
                <div className="mb-4">
                    <Input
                        value={search}
                        onChange={e => setSearch(e.target.value)}
                        placeholder="Search requirements to add as dependency..."
                        autoFocus
                    />
                    {search && searchResults.length > 0 && (
                        <div className="mt-1 max-h-40 overflow-y-auto rounded-md border">
                            {searchResults.slice(0, 10).map(r => (
                                <button
                                    key={r.id}
                                    type="button"
                                    onClick={() => addMutation.mutate(r.id)}
                                    disabled={addMutation.isPending}
                                    className="hover:bg-muted w-full px-3 py-1.5 text-left text-sm transition-colors"
                                >
                                    {r.title}
                                </button>
                            ))}
                        </div>
                    )}
                </div>
            )}

            {dependsOn.length > 0 && (
                <div className="mb-3">
                    <h3 className="text-muted-foreground mb-1.5 text-xs font-medium uppercase tracking-wide">Depends on</h3>
                    <div className="space-y-1">
                        {dependsOn.map(dep => (
                            <div key={dep.id} className="flex items-center gap-2 rounded-md border px-3 py-2">
                                <Link
                                    to="/requirements/$requirementId"
                                    params={{ requirementId: dep.id }}
                                    className="hover:underline flex-1 text-sm"
                                >
                                    {dep.title}
                                </Link>
                                <Badge variant="outline" className={STATUS_STYLES[dep.status] ?? STATUS_STYLES.untested}>
                                    {dep.status}
                                </Badge>
                                <button
                                    onClick={() => removeMutation.mutate(dep.id)}
                                    disabled={removeMutation.isPending}
                                    className="text-muted-foreground hover:text-destructive transition-colors"
                                >
                                    <X className="size-3.5" />
                                </button>
                            </div>
                        ))}
                    </div>
                </div>
            )}

            {dependedOnBy.length > 0 && (
                <div>
                    <h3 className="text-muted-foreground mb-1.5 text-xs font-medium uppercase tracking-wide">Depended on by</h3>
                    <div className="space-y-1">
                        {dependedOnBy.map(dep => (
                            <div key={dep.id} className="flex items-center gap-2 rounded-md border px-3 py-2">
                                <Link
                                    to="/requirements/$requirementId"
                                    params={{ requirementId: dep.id }}
                                    className="hover:underline flex-1 text-sm"
                                >
                                    {dep.title}
                                </Link>
                                <Badge variant="outline" className={STATUS_STYLES[dep.status] ?? STATUS_STYLES.untested}>
                                    {dep.status}
                                </Badge>
                            </div>
                        ))}
                    </div>
                </div>
            )}

            {dependsOn.length === 0 && dependedOnBy.length === 0 && !showAdd && (
                <p className="text-muted-foreground text-sm">No dependencies.</p>
            )}
        </div>
    );
}
