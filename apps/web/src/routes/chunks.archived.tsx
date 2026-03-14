import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Archive, ArrowLeft, RotateCcw, Trash2 } from "lucide-react";
import { useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/chunks/archived")({
    component: ArchivedChunks,
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            return { session: null };
        }
    }
});

function ArchivedChunks() {
    const queryClient = useQueryClient();
    const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

    const archivedQuery = useQuery({
        queryKey: ["chunks-archived"],
        queryFn: async () => unwrapEden(await (api.api.chunks as any).archived.get())
    });

    const restoreMutation = useMutation({
        mutationFn: async (id: string) => {
            const { error } = await (api.api.chunks as any)[id].restore.post();
            if (error) throw new Error("Failed to restore chunk");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-archived"] });
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => {
            const { error } = await api.api.chunks({ id }).delete();
            if (error) throw new Error("Failed to delete chunk");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-archived"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
        }
    });

    const chunks = (archivedQuery.data as Array<{ id: string; title: string; type: string; archivedAt: string }>) ?? [];

    function handleRestore(id: string) {
        restoreMutation.mutate(id);
    }

    function handleDelete(id: string) {
        setDeleteTarget(id);
    }

    return (
        <div className="container mx-auto max-w-5xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <div className="flex items-center gap-3">
                    <Link to="/chunks" search={{}} className="text-muted-foreground hover:text-foreground">
                        <ArrowLeft className="size-5" />
                    </Link>
                    <div>
                        <h1 className="text-2xl font-bold tracking-tight">Archived Chunks</h1>
                        <p className="text-muted-foreground mt-1 text-xs">{chunks.length} archived chunks</p>
                    </div>
                </div>
            </div>

            {chunks.length === 0 ? (
                <div className="text-muted-foreground flex flex-col items-center gap-3 py-16 text-center">
                    <Archive className="size-12 opacity-30" />
                    <p className="text-sm">No archived chunks</p>
                    <Link to="/chunks" search={{}} className="text-primary text-sm hover:underline">
                        Back to chunks
                    </Link>
                </div>
            ) : (
                <Card>
                    {chunks.map(chunk => (
                        <CardPanel key={chunk.id} className="flex items-center justify-between gap-4 px-4 py-3">
                            <div className="min-w-0 flex-1">
                                <p className="truncate text-sm font-medium">{chunk.title}</p>
                                <div className="mt-1 flex items-center gap-2">
                                    <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                        {chunk.type}
                                    </Badge>
                                    {chunk.archivedAt && (
                                        <span className="text-muted-foreground text-xs">
                                            Archived {new Date(chunk.archivedAt).toLocaleDateString()}
                                        </span>
                                    )}
                                </div>
                            </div>
                            <div className="flex items-center gap-2">
                                <Button
                                    variant="outline"
                                    size="sm"
                                    onClick={() => handleRestore(chunk.id)}
                                    disabled={restoreMutation.isPending}
                                >
                                    <RotateCcw className="size-3.5" />
                                    Restore
                                </Button>
                                <Button
                                    variant="destructive"
                                    size="sm"
                                    onClick={() => handleDelete(chunk.id)}
                                    disabled={deleteMutation.isPending}
                                >
                                    <Trash2 className="size-3.5" />
                                    Delete
                                </Button>
                            </div>
                        </CardPanel>
                    ))}
                </Card>
            )}

            <ConfirmDialog
                open={deleteTarget !== null}
                onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}
                title="Delete chunk permanently"
                description="Permanently delete this chunk? This cannot be undone."
                confirmLabel="Delete"
                confirmVariant="destructive"
                onConfirm={() => {
                    if (deleteTarget) {
                        deleteMutation.mutate(deleteTarget);
                        setDeleteTarget(null);
                    }
                }}
                loading={deleteMutation.isPending}
            />
        </div>
    );
}
