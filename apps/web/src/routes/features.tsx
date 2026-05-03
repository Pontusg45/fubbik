import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
    Archive,
    GitMerge,
    Layers,
    MoreHorizontal,
    Plus,
    PowerOff,
    Trash2,
    Zap,
} from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { useApiQuery } from "@/hooks/use-api-query";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from "@/components/ui/dialog";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { PageContainer, PageEmpty, PageHeader, PageLoading } from "@/components/ui/page";
import { getUser } from "@/functions/get-user";
import { useActiveFeatures } from "@/features/feature-flags/use-active-features";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/features")({
    component: FeaturesPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    },
});

interface Feature {
    id: string;
    name: string;
    description: string | null;
    priority: number;
    status: string;
    color: string | null;
    deltaCount: number;
    createdAt: string;
    updatedAt: string;
}

type FeatureStatus = "active" | "inactive" | "archived" | "merged";

function statusBadgeVariant(status: string): "success" | "secondary" | "warning" | "outline" {
    switch (status) {
        case "active": return "success";
        case "archived": return "warning";
        case "merged": return "secondary";
        default: return "outline";
    }
}

function FeaturesPage() {
    const queryClient = useQueryClient();
    const { isActive, toggleFeature } = useActiveFeatures();

    // Create dialog state
    const [showCreate, setShowCreate] = useState(false);
    const [newName, setNewName] = useState("");
    const [newDescription, setNewDescription] = useState("");
    const [newColor, setNewColor] = useState("#8b5cf6");

    // Merge confirmation state
    const [mergeTarget, setMergeTarget] = useState<Feature | null>(null);

    // Archive / delete confirmation state
    const [archiveTarget, setArchiveTarget] = useState<Feature | null>(null);
    const [deleteTarget, setDeleteTarget] = useState<Feature | null>(null);

    const featuresQuery = useApiQuery<Feature[]>({
        queryKey: ["features"],
        queryFn: () => api.api.features.get({ query: {} }),
        fallback: [],
    });

    const createMutation = useMutation({
        mutationFn: async (body: { name: string; description?: string; color?: string }) =>
            unwrapEden(await api.api.features.post(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["features"] });
            setShowCreate(false);
            setNewName("");
            setNewDescription("");
            setNewColor("#8b5cf6");
            toast.success("Feature created");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to create feature";
            toast.error(msg);
        },
    });

    const patchMutation = useMutation({
        mutationFn: async ({ id, body }: { id: string; body: { status?: FeatureStatus; name?: string } }) =>
            unwrapEden(await api.api.features({ id }).patch(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["features"] });
            queryClient.invalidateQueries({ queryKey: ["features", "active"] });
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to update feature";
            toast.error(msg);
        },
    });

    const mergeMutation = useMutation({
        mutationFn: async (id: string) =>
            unwrapEden(await api.api.features({ id }).merge.post({})),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["features"] });
            queryClient.invalidateQueries({ queryKey: ["features", "active"] });
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            setMergeTarget(null);
            toast.success("Feature merged into chunks");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to merge feature";
            toast.error(msg);
        },
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) =>
            unwrapEden(await api.api.features({ id }).delete()),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["features"] });
            queryClient.invalidateQueries({ queryKey: ["features", "active"] });
            toast.success("Feature deleted");
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to delete feature";
            toast.error(msg);
        },
    });

    function handleCreateSubmit(e: React.FormEvent) {
        e.preventDefault();
        if (!newName.trim()) return;
        createMutation.mutate({
            name: newName.trim(),
            ...(newDescription.trim() ? { description: newDescription.trim() } : {}),
            color: newColor,
        });
    }

    function handleActivate(feature: Feature) {
        if (isActive(feature.id)) {
            toggleFeature(feature.id);
            toast.success(`"${feature.name}" deactivated`);
        } else {
            if (feature.status !== "active") {
                patchMutation.mutate(
                    { id: feature.id, body: { status: "active" } },
                    {
                        onSuccess: () => {
                            toggleFeature(feature.id);
                            toast.success(`"${feature.name}" activated`);
                        },
                    }
                );
            } else {
                toggleFeature(feature.id);
                toast.success(`"${feature.name}" activated`);
            }
        }
    }

    function handleDeactivate(feature: Feature) {
        if (isActive(feature.id)) {
            toggleFeature(feature.id);
        }
        patchMutation.mutate(
            { id: feature.id, body: { status: "inactive" } },
            { onSuccess: () => toast.success(`"${feature.name}" deactivated`) }
        );
    }

    function handleArchive(feature: Feature) {
        patchMutation.mutate(
            { id: feature.id, body: { status: "archived" } },
            {
                onSuccess: () => {
                    setArchiveTarget(null);
                    toast.success(`"${feature.name}" archived`);
                },
            }
        );
    }

    const features = Array.isArray(featuresQuery.data) ? featuresQuery.data : [];
    const activeCount = features.filter(f => f.status === "active").length;

    return (
        <PageContainer maxWidth="4xl">
            <PageHeader
                icon={Layers}
                title="Features"
                count={features.length}
                actions={
                    <Button size="sm" onClick={() => setShowCreate(true)}>
                        <Plus className="size-3.5" />
                        New Feature
                    </Button>
                }
            />

            {/* Create dialog */}
            <Dialog open={showCreate} onOpenChange={open => { if (!open) { setShowCreate(false); setNewName(""); setNewDescription(""); setNewColor("#8b5cf6"); } }}>
                <DialogContent className="max-w-md">
                    <DialogHeader>
                        <DialogTitle>New Feature</DialogTitle>
                        <DialogDescription>
                            Features let you track parallel work streams with per-chunk deltas that can be merged when ready.
                        </DialogDescription>
                    </DialogHeader>
                    <form id="create-feature-form" onSubmit={handleCreateSubmit}>
                        <div className="space-y-4 py-2">
                            <div className="space-y-1.5">
                                <label htmlFor="feature-name" className="text-sm font-medium">Name</label>
                                <Input
                                    id="feature-name"
                                    placeholder="e.g. dark-mode-refactor"
                                    value={newName}
                                    onChange={e => setNewName(e.target.value)}
                                    autoFocus
                                    required
                                />
                            </div>
                            <div className="space-y-1.5">
                                <label htmlFor="feature-desc" className="text-sm font-medium">
                                    Description <span className="text-muted-foreground font-normal">(optional)</span>
                                </label>
                                <textarea
                                    id="feature-desc"
                                    placeholder="Short description of what this feature changes…"
                                    value={newDescription}
                                    onChange={e => setNewDescription(e.target.value)}
                                    rows={3}
                                    className="bg-background focus:ring-ring w-full rounded-lg border px-3 py-2 text-sm focus:ring-2 focus:outline-none resize-none"
                                />
                            </div>
                            <div className="flex items-center gap-3">
                                <label htmlFor="feature-color" className="text-sm font-medium">Color</label>
                                <input
                                    id="feature-color"
                                    type="color"
                                    value={newColor}
                                    onChange={e => setNewColor(e.target.value)}
                                    className="size-8 cursor-pointer rounded border p-0.5"
                                />
                            </div>
                        </div>
                    </form>
                    <DialogFooter>
                        <Button
                            variant="ghost"
                            onClick={() => { setShowCreate(false); setNewName(""); setNewDescription(""); setNewColor("#8b5cf6"); }}
                        >
                            Cancel
                        </Button>
                        <Button
                            type="submit"
                            form="create-feature-form"
                            disabled={!newName.trim() || createMutation.isPending}
                        >
                            {createMutation.isPending ? "Creating…" : "Create"}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Merge confirmation dialog */}
            <Dialog
                open={mergeTarget !== null}
                onOpenChange={open => { if (!open) setMergeTarget(null); }}
            >
                <DialogContent className="max-w-md">
                    <DialogHeader>
                        <DialogTitle>Merge feature</DialogTitle>
                        <DialogDescription>
                            All {mergeTarget?.deltaCount ?? 0} chunk delta{(mergeTarget?.deltaCount ?? 0) !== 1 ? "s" : ""} in{" "}
                            <span className="font-medium text-foreground">{mergeTarget?.name}</span> will be applied to
                            their chunks and the feature will be marked as merged. This cannot be undone.
                        </DialogDescription>
                    </DialogHeader>
                    <DialogFooter>
                        <Button variant="ghost" onClick={() => setMergeTarget(null)}>
                            Cancel
                        </Button>
                        <Button
                            variant="destructive"
                            disabled={mergeMutation.isPending}
                            onClick={() => { if (mergeTarget) mergeMutation.mutate(mergeTarget.id); }}
                        >
                            {mergeMutation.isPending ? "Merging…" : "Merge"}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Archive confirmation */}
            <ConfirmDialog
                open={archiveTarget !== null}
                onOpenChange={open => { if (!open) setArchiveTarget(null); }}
                title="Archive feature"
                description={archiveTarget ? `Archive "${archiveTarget.name}"? The feature will be hidden from active lists but its deltas are preserved.` : ""}
                confirmLabel="Archive"
                onConfirm={() => { if (archiveTarget) handleArchive(archiveTarget); }}
                loading={patchMutation.isPending}
            />

            {/* Delete confirmation */}
            <ConfirmDialog
                open={deleteTarget !== null}
                onOpenChange={open => { if (!open) setDeleteTarget(null); }}
                title="Delete feature"
                description={deleteTarget ? `Delete "${deleteTarget.name}"? All ${deleteTarget.deltaCount} chunk delta${deleteTarget.deltaCount !== 1 ? "s" : ""} will also be permanently removed.` : ""}
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

            {/* Feature list */}
            {featuresQuery.isLoading ? (
                <PageLoading count={5} />
            ) : features.length === 0 ? (
                <PageEmpty
                    icon={Layers}
                    title="No features yet"
                    description="Features let you track parallel work streams with chunk-level deltas that can be merged when ready."
                    action={<Button onClick={() => setShowCreate(true)}>Create Feature</Button>}
                />
            ) : (
                <div className="space-y-2">
                    {activeCount > 0 && (
                        <p className="text-muted-foreground mb-3 text-xs">
                            {activeCount} active feature{activeCount !== 1 ? "s" : ""} — chunk content may differ from base when browsing.
                        </p>
                    )}
                    {features.map(feature => (
                        <FeatureCard
                            key={feature.id}
                            feature={feature}
                            isActive={isActive(feature.id)}
                            onActivate={() => handleActivate(feature)}
                            onDeactivate={() => handleDeactivate(feature)}
                            onMerge={() => setMergeTarget(feature)}
                            onArchive={() => setArchiveTarget(feature)}
                            onDelete={() => setDeleteTarget(feature)}
                        />
                    ))}
                </div>
            )}
        </PageContainer>
    );
}

interface FeatureCardProps {
    feature: Feature;
    isActive: boolean;
    onActivate: () => void;
    onDeactivate: () => void;
    onMerge: () => void;
    onArchive: () => void;
    onDelete: () => void;
}

function FeatureCard({
    feature,
    isActive,
    onActivate,
    onDeactivate,
    onMerge,
    onArchive,
    onDelete,
}: FeatureCardProps) {
    const isMerged = feature.status === "merged";
    const isArchived = feature.status === "archived";
    const canMerge = !isMerged && !isArchived && feature.deltaCount > 0;

    return (
        <div className="group flex items-center gap-4 rounded-lg border px-4 py-3 transition-colors hover:bg-muted/40">
            {/* Colored dot */}
            <div
                className="size-3 shrink-0 rounded-full"
                style={{ backgroundColor: feature.color ?? "#8b5cf6" }}
            />

            {/* Name + description */}
            <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium">{feature.name}</span>
                    <Badge variant={statusBadgeVariant(feature.status)} size="sm">
                        {feature.status}
                    </Badge>
                    <Badge variant="outline" size="sm">
                        priority {feature.priority}
                    </Badge>
                    {isActive && (
                        <Badge variant="info" size="sm">
                            <Zap className="size-3" />
                            active
                        </Badge>
                    )}
                </div>
                {feature.description && (
                    <p className="text-muted-foreground mt-0.5 text-sm line-clamp-1">{feature.description}</p>
                )}
            </div>

            {/* Delta count */}
            <span className="text-muted-foreground shrink-0 text-xs tabular-nums" title={`${feature.deltaCount} chunk delta${feature.deltaCount !== 1 ? "s" : ""}`}>
                {feature.deltaCount} delta{feature.deltaCount !== 1 ? "s" : ""}
            </span>

            {/* Actions */}
            <div className="flex shrink-0 items-center gap-1">
                {/* Quick activate/deactivate button */}
                {!isMerged && !isArchived && (
                    <Button
                        size="sm"
                        variant={isActive ? "secondary" : "outline"}
                        onClick={isActive ? onDeactivate : onActivate}
                        title={isActive ? "Deactivate feature" : "Activate feature"}
                        className="h-7 px-2"
                    >
                        {isActive ? (
                            <>
                                <PowerOff className="size-3.5" />
                                <span className="sr-only sm:not-sr-only sm:ml-1 sm:text-xs">Deactivate</span>
                            </>
                        ) : (
                            <>
                                <Zap className="size-3.5" />
                                <span className="sr-only sm:not-sr-only sm:ml-1 sm:text-xs">Activate</span>
                            </>
                        )}
                    </Button>
                )}

                <DropdownMenu>
                    <DropdownMenuTrigger
                        render={
                            <button
                                aria-label={`Actions for ${feature.name}`}
                                className="text-muted-foreground hover:text-foreground flex size-7 items-center justify-center rounded-md transition-colors"
                            >
                                <MoreHorizontal className="size-4" />
                            </button>
                        }
                    />
                    <DropdownMenuContent align="end">
                        {canMerge && (
                            <DropdownMenuItem onClick={onMerge}>
                                <GitMerge className="size-3.5" />
                                Merge into chunks
                            </DropdownMenuItem>
                        )}
                        {!isMerged && !isArchived && (
                            <DropdownMenuItem onClick={onArchive}>
                                <Archive className="size-3.5" />
                                Archive
                            </DropdownMenuItem>
                        )}
                        {(canMerge || (!isMerged && !isArchived)) && <DropdownMenuSeparator />}
                        <DropdownMenuItem onClick={onDelete} className="text-destructive">
                            <Trash2 className="size-3.5" />
                            Delete
                        </DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>
            </div>
        </div>
    );
}
