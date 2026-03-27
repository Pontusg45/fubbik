import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { ChevronDown, ChevronRight, Folder, Layers, Plus, Trash2, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { PageContainer, PageEmpty, PageHeader, PageLoading } from "@/components/ui/page";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/workspaces")({
    component: WorkspacesPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest access
        }
        return { session };
    }
});

function WorkspacesPage() {
    const queryClient = useQueryClient();
    const [name, setName] = useState("");
    const [description, setDescription] = useState("");
    const [expandedId, setExpandedId] = useState<string | null>(null);
    const [deleteTarget, setDeleteTarget] = useState<{ id: string; name: string } | null>(null);

    const workspacesQuery = useQuery({
        queryKey: ["workspaces"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.workspaces.get());
            } catch {
                return [];
            }
        }
    });

    const codebasesQuery = useQuery({
        queryKey: ["codebases"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.codebases.get());
            } catch {
                return [];
            }
        },
        staleTime: 60_000
    });

    const createMutation = useMutation({
        mutationFn: async (body: { name: string; description?: string }) => {
            return unwrapEden(await api.api.workspaces.post(body));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["workspaces"] });
            setName("");
            setDescription("");
            toast.success("Workspace created");
        },
        onError: () => toast.error("Failed to create workspace")
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => {
            return unwrapEden(await api.api.workspaces({ id }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["workspaces"] });
            toast.success("Workspace deleted");
        },
        onError: () => toast.error("Failed to delete workspace")
    });

    const addCodebaseMutation = useMutation({
        mutationFn: async ({ workspaceId, codebaseId }: { workspaceId: string; codebaseId: string }) => {
            return unwrapEden(await api.api.workspaces({ id: workspaceId }).codebases.post({ codebaseId }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["workspaces"] });
            queryClient.invalidateQueries({ queryKey: ["workspace-detail"] });
            toast.success("Codebase added");
        },
        onError: () => toast.error("Failed to add codebase")
    });

    const removeCodebaseMutation = useMutation({
        mutationFn: async ({ workspaceId, codebaseId }: { workspaceId: string; codebaseId: string }) => {
            return unwrapEden(await api.api.workspaces({ id: workspaceId }).codebases({ codebaseId }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["workspaces"] });
            queryClient.invalidateQueries({ queryKey: ["workspace-detail"] });
            toast.success("Codebase removed");
        },
        onError: () => toast.error("Failed to remove codebase")
    });

    const workspaces = Array.isArray(workspacesQuery.data) ? workspacesQuery.data : [];
    const codebases = Array.isArray(codebasesQuery.data) ? codebasesQuery.data : [];

    function handleCreate(e: React.FormEvent) {
        e.preventDefault();
        if (!name.trim()) return;
        createMutation.mutate({
            name: name.trim(),
            ...(description.trim() ? { description: description.trim() } : {})
        });
    }

    return (
        <PageContainer>
            <PageHeader
                icon={Layers}
                title="Workspaces"
                count={workspaces.length}
            />

            <Card className="mb-6">
                <CardPanel className="p-6">
                    <form onSubmit={handleCreate} className="flex flex-col gap-3">
                        <h2 className="text-sm font-medium">Create Workspace</h2>
                        <div className="flex flex-col gap-2 sm:flex-row">
                            <Input
                                placeholder="Name"
                                value={name}
                                onChange={e => setName(e.target.value)}
                                required
                                className="flex-1"
                            />
                            <Input
                                placeholder="Description (optional)"
                                value={description}
                                onChange={e => setDescription(e.target.value)}
                                className="flex-1"
                            />
                            <Button type="submit" size="sm" disabled={createMutation.isPending || !name.trim()}>
                                <Plus className="mr-1 size-4" />
                                Create
                            </Button>
                        </div>
                    </form>
                </CardPanel>
            </Card>

            <Card>
                <CardPanel className="p-6">
                    {workspacesQuery.isLoading ? (
                        <PageLoading count={3} />
                    ) : workspaces.length === 0 ? (
                        <PageEmpty
                            icon={Layers}
                            title="No workspaces"
                            description="Create a workspace to group codebases together."
                            action={
                                <Button onClick={() => document.querySelector<HTMLInputElement>('input[placeholder="Name"]')?.focus()}>
                                    Create Workspace
                                </Button>
                            }
                        />
                    ) : (
                        <div className="divide-y">
                            {workspaces.map((ws: any) => (
                                <WorkspaceRow
                                    key={ws.id}
                                    workspace={ws}
                                    expanded={expandedId === ws.id}
                                    onToggle={() => setExpandedId(expandedId === ws.id ? null : ws.id)}
                                    onDelete={() => setDeleteTarget({ id: ws.id, name: ws.name })}
                                    codebases={codebases}
                                    onAddCodebase={(codebaseId: string) =>
                                        addCodebaseMutation.mutate({ workspaceId: ws.id, codebaseId })
                                    }
                                    onRemoveCodebase={(codebaseId: string) =>
                                        removeCodebaseMutation.mutate({ workspaceId: ws.id, codebaseId })
                                    }
                                />
                            ))}
                        </div>
                    )}
                </CardPanel>
            </Card>

            <ConfirmDialog
                open={deleteTarget !== null}
                onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}
                title="Delete workspace"
                description={deleteTarget ? `Delete workspace "${deleteTarget.name}"?` : ""}
                confirmLabel="Delete"
                confirmVariant="destructive"
                onConfirm={() => {
                    if (deleteTarget) {
                        deleteMutation.mutate(deleteTarget.id);
                        setDeleteTarget(null);
                    }
                }}
                loading={deleteMutation.isPending}
            />
        </PageContainer>
    );
}

function WorkspaceRow({
    workspace,
    expanded,
    onToggle,
    onDelete,
    codebases,
    onAddCodebase,
    onRemoveCodebase
}: {
    workspace: any;
    expanded: boolean;
    onToggle: () => void;
    onDelete: () => void;
    codebases: any[];
    onAddCodebase: (codebaseId: string) => void;
    onRemoveCodebase: (codebaseId: string) => void;
}) {
    const detailQuery = useQuery({
        queryKey: ["workspace-detail", workspace.id],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.workspaces({ id: workspace.id }).get());
            } catch {
                return null;
            }
        },
        enabled: expanded
    });

    const detail = detailQuery.data as any;
    const workspaceCodebases = detail?.codebases ?? [];
    const workspaceCodebaseIds = new Set(workspaceCodebases.map((c: any) => c.id));
    const availableCodebases = codebases.filter(c => !workspaceCodebaseIds.has(c.id));

    return (
        <div className="py-3 first:pt-0 last:pb-0">
            <div className="flex items-center justify-between">
                <button onClick={onToggle} className="flex min-w-0 flex-1 items-center gap-2 text-left">
                    {expanded ? <ChevronDown className="size-4 shrink-0" /> : <ChevronRight className="size-4 shrink-0" />}
                    <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                            <p className="font-medium">{workspace.name}</p>
                            <Badge variant="secondary" size="sm" className="text-[10px]">
                                {workspace.codebaseCount ?? 0} codebases
                            </Badge>
                        </div>
                        {workspace.description && (
                            <p className="text-muted-foreground mt-0.5 truncate text-sm">{workspace.description}</p>
                        )}
                    </div>
                </button>
                <Button variant="ghost" size="sm" onClick={onDelete}>
                    <Trash2 className="size-4" />
                </Button>
            </div>

            {expanded && (
                <div className="mt-3 ml-6 space-y-2">
                    {detailQuery.isLoading ? (
                        <p className="text-muted-foreground text-sm">Loading...</p>
                    ) : workspaceCodebases.length === 0 ? (
                        <p className="text-muted-foreground text-sm">No codebases in this workspace.</p>
                    ) : (
                        <div className="space-y-1">
                            {workspaceCodebases.map((cb: any) => (
                                <div key={cb.id} className="flex items-center justify-between rounded-md bg-muted/50 px-3 py-1.5">
                                    <div className="flex items-center gap-2">
                                        <Folder className="text-muted-foreground size-3.5" />
                                        <span className="text-sm">{cb.name}</span>
                                    </div>
                                    <button
                                        onClick={() => onRemoveCodebase(cb.id)}
                                        className="text-muted-foreground hover:text-destructive"
                                    >
                                        <X className="size-3.5" />
                                    </button>
                                </div>
                            ))}
                        </div>
                    )}

                    {availableCodebases.length > 0 && (
                        <Popover>
                            <PopoverTrigger
                                render={<Button variant="outline" size="sm" />}
                            >
                                <Plus className="mr-1 size-3" />
                                Add Codebase
                            </PopoverTrigger>
                            <PopoverContent align="start" className="w-56 p-1">
                                {availableCodebases.map((cb: any) => (
                                    <button
                                        key={cb.id}
                                        onClick={() => onAddCodebase(cb.id)}
                                        className="hover:bg-muted flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors"
                                    >
                                        <Folder className="text-muted-foreground size-3.5" />
                                        {cb.name}
                                    </button>
                                ))}
                            </PopoverContent>
                        </Popover>
                    )}
                </div>
            )}
        </div>
    );
}
