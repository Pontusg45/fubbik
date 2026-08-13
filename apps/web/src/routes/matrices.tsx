import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Grid3X3, Plus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { PageContainer, PageEmpty, PageHeader, PageLoading } from "@/components/ui/page";
import { getUser } from "@/functions/get-user";
import { useApiQuery } from "@/hooks/use-api-query";
import { legacyApi } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/matrices")({
    component: MatricesPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

interface Matrix {
    id: string;
    name: string;
    layer: string;
    description: string | null;
    spaceId: string | null;
    createdAt: string;
    updatedAt: string;
}

function layerBadgeVariant(layer: string): "info" | "warning" {
    return layer === "invariant" ? "info" : "warning";
}

function MatricesPage() {
    const navigate = useNavigate();
    const queryClient = useQueryClient();

    const [showCreate, setShowCreate] = useState(false);
    const [newName, setNewName] = useState("");
    const [newLayer, setNewLayer] = useState<"invariant" | "contract">("invariant");
    const [newDescription, setNewDescription] = useState("");

    const matricesQuery = useApiQuery<Matrix[]>({
        queryKey: ["matrices"],
        queryFn: () => legacyApi.api.matrices.get({ query: {} }),
        fallback: []
    });

    const createMutation = useMutation({
        mutationFn: async (body: { name: string; layer: "invariant" | "contract"; description?: string }) =>
            unwrapEden(await legacyApi.api.matrices.post(body)),
        onSuccess: data => {
            queryClient.invalidateQueries({ queryKey: ["matrices"] });
            setShowCreate(false);
            resetForm();
            toast.success("Matrix created");
            if (data && typeof data === "object" && "id" in data) {
                navigate({ to: "/matrices/$matrixId", params: { matrixId: (data as { id: string }).id } });
            }
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to create matrix";
            toast.error(msg);
        }
    });

    function resetForm() {
        setNewName("");
        setNewLayer("invariant");
        setNewDescription("");
    }

    function handleCreateSubmit(e: React.FormEvent) {
        e.preventDefault();
        if (!newName.trim()) return;
        createMutation.mutate({
            name: newName.trim(),
            layer: newLayer,
            ...(newDescription.trim() ? { description: newDescription.trim() } : {})
        });
    }

    const matrices = Array.isArray(matricesQuery.data) ? matricesQuery.data : [];

    return (
        <PageContainer maxWidth="4xl">
            <PageHeader
                icon={Grid3X3}
                title="Matrices"
                count={matrices.length}
                actions={
                    <Button size="sm" onClick={() => setShowCreate(true)}>
                        <Plus className="size-3.5" />
                        New Matrix
                    </Button>
                }
            />

            <Dialog
                open={showCreate}
                onOpenChange={open => {
                    if (!open) {
                        setShowCreate(false);
                        resetForm();
                    }
                }}
            >
                <DialogContent className="max-w-md">
                    <DialogHeader>
                        <DialogTitle>New Matrix</DialogTitle>
                        <DialogDescription>
                            A behavioral specification matrix maps rules against dimensions to track coverage.
                        </DialogDescription>
                    </DialogHeader>
                    <form id="create-matrix-form" onSubmit={handleCreateSubmit}>
                        <div className="space-y-4 py-2">
                            <div className="space-y-1.5">
                                <label htmlFor="matrix-name" className="text-sm font-medium">
                                    Name
                                </label>
                                <Input
                                    id="matrix-name"
                                    placeholder="e.g. API Error Handling"
                                    value={newName}
                                    onChange={e => setNewName(e.target.value)}
                                    autoFocus
                                    required
                                />
                            </div>
                            <div className="space-y-1.5">
                                <label htmlFor="matrix-layer" className="text-sm font-medium">
                                    Layer
                                </label>
                                <select
                                    id="matrix-layer"
                                    value={newLayer}
                                    onChange={e => setNewLayer(e.target.value as "invariant" | "contract")}
                                    className="bg-background border-input focus:ring-ring w-full rounded-lg border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                >
                                    <option value="invariant">Invariant</option>
                                    <option value="contract">Contract</option>
                                </select>
                            </div>
                            <div className="space-y-1.5">
                                <label htmlFor="matrix-desc" className="text-sm font-medium">
                                    Description <span className="text-muted-foreground font-normal">(optional)</span>
                                </label>
                                <textarea
                                    id="matrix-desc"
                                    placeholder="What does this matrix track?"
                                    value={newDescription}
                                    onChange={e => setNewDescription(e.target.value)}
                                    rows={3}
                                    className="bg-background focus:ring-ring w-full resize-none rounded-lg border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                />
                            </div>
                        </div>
                    </form>
                    <DialogFooter>
                        <Button
                            variant="ghost"
                            onClick={() => {
                                setShowCreate(false);
                                resetForm();
                            }}
                        >
                            Cancel
                        </Button>
                        <Button type="submit" form="create-matrix-form" disabled={!newName.trim() || createMutation.isPending}>
                            {createMutation.isPending ? "Creating..." : "Create"}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {matricesQuery.isLoading ? (
                <PageLoading count={4} />
            ) : matrices.length === 0 ? (
                <PageEmpty
                    icon={Grid3X3}
                    title="No matrices yet"
                    description="Create a behavioral specification matrix to map rules against dimensions and track coverage."
                    action={<Button onClick={() => setShowCreate(true)}>Create Matrix</Button>}
                />
            ) : (
                <div className="space-y-2">
                    {matrices.map(matrix => (
                        <button
                            key={matrix.id}
                            type="button"
                            onClick={() => navigate({ to: "/matrices/$matrixId", params: { matrixId: matrix.id } })}
                            className="group hover:bg-muted/40 flex w-full items-center gap-4 rounded-lg border px-4 py-3 text-left transition-colors"
                        >
                            <Grid3X3 className="text-muted-foreground size-4 shrink-0" />
                            <div className="min-w-0 flex-1">
                                <div className="flex flex-wrap items-center gap-2">
                                    <span className="font-medium">{matrix.name}</span>
                                    <Badge variant={layerBadgeVariant(matrix.layer)} size="sm">
                                        {matrix.layer}
                                    </Badge>
                                </div>
                                {matrix.description && (
                                    <p className="text-muted-foreground mt-0.5 line-clamp-1 text-sm">{matrix.description}</p>
                                )}
                            </div>
                        </button>
                    ))}
                </div>
            )}
        </PageContainer>
    );
}
