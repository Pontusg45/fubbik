import { createFileRoute } from "@tanstack/react-router";
import { FolderGit2, GitBranch, Plus, RotateCcw, Trash2 } from "lucide-react";
import { useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { PageContainer, PageEmpty, PageHeader, PageLoading } from "@/components/ui/page";
import { useApiMutation } from "@/hooks/use-api-mutation";
import { useApiQuery } from "@/hooks/use-api-query";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/spaces")({
    component: SpacesPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

type ConfirmAction = {
    type: "reset" | "delete";
    id: string;
    name: string;
};

function SpacesPage() {
    const [name, setName] = useState("");
    const [remoteUrl, setRemoteUrl] = useState("");
    const [confirmAction, setConfirmAction] = useState<ConfirmAction | null>(null);

    const spacesQuery = useApiQuery<any[]>({
        queryKey: ["spaces"],
        queryFn: () => api.api.spaces.get(),
        fallback: [],
    });

    const createMutation = useApiMutation<unknown, { name: string; remoteUrl?: string }>({
        mutationFn: body => api.api.spaces.post(body),
        invalidate: ["spaces"],
        successToast: false,
        onSuccess: () => {
            setName("");
            setRemoteUrl("");
        },
    });

    const deleteMutation = useApiMutation<unknown, string>({
        mutationFn: id => api.api.spaces({ id }).delete(),
        invalidate: ["spaces", "chunks", "graph", "stats"],
        successToast: "Space deleted",
    });

    const resetMutation = useApiMutation<
        { chunksDeleted: number; docsDeleted: number; plansDeleted: number; requirementsDeleted: number },
        string
    >({
        mutationFn: async (id) => {
            const result = unwrapEden(await api.api.spaces({ id }).reset.post());
            return result as any;
        },
        invalidate: ["spaces", "chunks", "graph", "stats"],
        successToast: (data) => {
            const parts = [];
            if (data.chunksDeleted) parts.push(`${data.chunksDeleted} chunks`);
            if (data.docsDeleted) parts.push(`${data.docsDeleted} docs`);
            if (data.plansDeleted) parts.push(`${data.plansDeleted} plans`);
            if (data.requirementsDeleted) parts.push(`${data.requirementsDeleted} requirements`);
            return parts.length > 0 ? `Reset: removed ${parts.join(", ")}` : "Space reset (was already empty)";
        },
    });

    const spaces = Array.isArray(spacesQuery.data) ? spacesQuery.data : [];

    function handleCreate(e: React.FormEvent) {
        e.preventDefault();
        if (!name.trim()) return;
        createMutation.mutate({
            name: name.trim(),
            ...(remoteUrl.trim() ? { remoteUrl: remoteUrl.trim() } : {})
        });
    }

    function handleConfirm() {
        if (!confirmAction) return;
        if (confirmAction.type === "delete") {
            deleteMutation.mutate(confirmAction.id);
        } else {
            resetMutation.mutate(confirmAction.id);
        }
        setConfirmAction(null);
    }

    const isPending = deleteMutation.isPending || resetMutation.isPending;

    return (
        <PageContainer>
            <PageHeader
                icon={FolderGit2}
                title="Spaces"
                count={spaces.length}
            />

            <Card className="mb-6">
                <CardPanel className="p-6">
                    <form onSubmit={handleCreate} className="flex flex-col gap-3">
                        <h2 className="text-sm font-medium">Add Space</h2>
                        <div className="flex flex-col gap-2 sm:flex-row">
                            <Input
                                placeholder="Name"
                                value={name}
                                onChange={e => setName(e.target.value)}
                                required
                                className="flex-1"
                            />
                            <Input
                                placeholder="Remote URL (optional)"
                                value={remoteUrl}
                                onChange={e => setRemoteUrl(e.target.value)}
                                className="flex-1"
                            />
                            <Button type="submit" size="sm" disabled={createMutation.isPending || !name.trim()}>
                                <Plus className="mr-1 size-4" />
                                Add
                            </Button>
                        </div>
                    </form>
                </CardPanel>
            </Card>

            <Card>
                <CardPanel className="p-6">
                    {spacesQuery.isLoading ? (
                        <PageLoading count={3} />
                    ) : spaces.length === 0 ? (
                        <PageEmpty
                            icon={GitBranch}
                            title="No spaces"
                            description="Add a space to scope chunks to specific projects."
                            action={<Button onClick={() => document.querySelector<HTMLInputElement>('input[placeholder="Name"]')?.focus()}>Add Space</Button>}
                        />
                    ) : (
                        <div className="divide-y">
                            {spaces.map(c => (
                                <div key={c.id} className="flex items-center justify-between py-3 first:pt-0 last:pb-0">
                                    <div className="min-w-0 flex-1">
                                        <p className="font-medium">{c.name}</p>
                                        {c.remoteUrl && (
                                            <p className="text-muted-foreground truncate text-sm">{c.remoteUrl}</p>
                                        )}
                                        {"localPaths" in c &&
                                            Array.isArray(c.localPaths) &&
                                            c.localPaths.length > 0 && (
                                                <p className="text-muted-foreground text-xs">
                                                    {c.localPaths.join(", ")}
                                                </p>
                                            )}
                                    </div>
                                    <div className="flex gap-1">
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => setConfirmAction({ type: "reset", id: c.id, name: c.name })}
                                            disabled={isPending}
                                            title="Reset — delete all chunks, docs, plans, and requirements"
                                        >
                                            <RotateCcw className="size-4" />
                                        </Button>
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => setConfirmAction({ type: "delete", id: c.id, name: c.name })}
                                            disabled={isPending}
                                            title="Delete space and all its data"
                                        >
                                            <Trash2 className="size-4" />
                                        </Button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </CardPanel>
            </Card>

            <ConfirmDialog
                open={confirmAction !== null}
                onOpenChange={(open) => { if (!open) setConfirmAction(null); }}
                title={confirmAction?.type === "reset" ? "Reset space" : "Delete space"}
                description={
                    confirmAction?.type === "reset"
                        ? `This will delete all chunks, documents, plans, and requirements in "${confirmAction.name}". The space itself will be kept.`
                        : confirmAction
                        ? `This will delete "${confirmAction.name}" and all its chunks, documents, plans, and requirements. This cannot be undone.`
                        : ""
                }
                confirmLabel={confirmAction?.type === "reset" ? "Reset" : "Delete"}
                confirmVariant="destructive"
                onConfirm={handleConfirm}
                loading={isPending}
            />
        </PageContainer>
    );
}
