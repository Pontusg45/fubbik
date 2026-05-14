import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Grid3X3, Plus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { BackLink } from "@/components/back-link";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { PageContainer, PageLoading } from "@/components/ui/page";
import { CellPanel } from "@/features/matrices/cell-panel";
import { MatrixGrid, type Dimension, type Rule, type ViewCell } from "@/features/matrices/matrix-grid";
import { useApiQuery } from "@/hooks/use-api-query";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/matrices_/$matrixId")({
    component: MatrixDetailPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    },
});

interface MatrixView {
    matrix: {
        id: string;
        name: string;
        layer: string;
        description: string | null;
    };
    dimensions: Dimension[];
    rules: Rule[];
    cells: Record<string, ViewCell | null>;
    summary: {
        specified: number;
        unspecified: number;
        violated: number;
        total: number;
    };
}

interface SelectedCell {
    cellId: string;
    ruleId: string;
    dimensionId: string;
    ruleTitle: string;
    dimensionName: string;
}

function MatrixDetailPage() {
    const { matrixId } = Route.useParams();
    const queryClient = useQueryClient();

    // Add dimension / rule form state
    const [newDimName, setNewDimName] = useState("");
    const [newRuleTitle, setNewRuleTitle] = useState("");
    const [newRuleCategory, setNewRuleCategory] = useState("");

    // Cell panel state
    const [selectedCell, setSelectedCell] = useState<SelectedCell | null>(null);

    const viewQuery = useApiQuery<MatrixView>({
        queryKey: ["matrix-view", matrixId],
        queryFn: () => api.api.matrices({ id: matrixId }).view.get(),
    });

    const addDimensionMutation = useMutation({
        mutationFn: async (name: string) =>
            unwrapEden(await api.api.matrices({ id: matrixId }).dimensions.post({ name })),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            setNewDimName("");
            toast.success("Dimension added");
        },
        onError: (err: unknown) => {
            toast.error(err instanceof Error ? err.message : "Failed to add dimension");
        },
    });

    const addRuleMutation = useMutation({
        mutationFn: async (body: { title: string; category?: string }) =>
            unwrapEden(await api.api.matrices({ id: matrixId }).rules.post(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            setNewRuleTitle("");
            setNewRuleCategory("");
            toast.success("Rule added");
        },
        onError: (err: unknown) => {
            toast.error(err instanceof Error ? err.message : "Failed to add rule");
        },
    });

    const toggleCellMutation = useMutation({
        mutationFn: async (body: { ruleId: string; dimensionId: string }) =>
            unwrapEden(await api.api.matrices({ id: matrixId }).cells.put(body)),
        onSuccess: (data) => {
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            if (data && typeof data === "object" && "action" in data) {
                const action = (data as { action: string }).action;
                toast.success(action === "created" ? "Cell created" : "Cell removed");
            }
        },
        onError: (err: unknown) => {
            toast.error(err instanceof Error ? err.message : "Failed to toggle cell");
        },
    });

    function handleAddDimension(e: React.FormEvent) {
        e.preventDefault();
        if (!newDimName.trim()) return;
        addDimensionMutation.mutate(newDimName.trim());
    }

    function handleAddRule(e: React.FormEvent) {
        e.preventDefault();
        if (!newRuleTitle.trim()) return;
        addRuleMutation.mutate({
            title: newRuleTitle.trim(),
            ...(newRuleCategory.trim() ? { category: newRuleCategory.trim() } : {}),
        });
    }

    function handleCellClick(cell: ViewCell, ruleId: string, dimensionId: string) {
        const view = viewQuery.data;
        if (!view) return;
        const rule = view.rules.find(r => r.id === ruleId);
        const dim = view.dimensions.find(d => d.id === dimensionId);
        if (!rule || !dim) return;
        setSelectedCell({
            cellId: cell.id,
            ruleId,
            dimensionId,
            ruleTitle: rule.title,
            dimensionName: dim.name,
        });
    }

    function handleToggleCell(ruleId: string, dimensionId: string) {
        // Close panel if it was for this cell
        if (selectedCell?.ruleId === ruleId && selectedCell?.dimensionId === dimensionId) {
            setSelectedCell(null);
        }
        toggleCellMutation.mutate({ ruleId, dimensionId });
    }

    if (viewQuery.isLoading) {
        return (
            <PageContainer maxWidth="6xl">
                <BackLink to="/matrices" label="Matrices" />
                <PageLoading count={6} />
            </PageContainer>
        );
    }

    if (viewQuery.error || !viewQuery.data) {
        return (
            <PageContainer maxWidth="6xl">
                <BackLink to="/matrices" label="Matrices" />
                <div className="text-muted-foreground py-12 text-center">
                    Failed to load matrix.
                </div>
            </PageContainer>
        );
    }

    const { matrix, dimensions, rules, cells, summary } = viewQuery.data;

    return (
        <PageContainer maxWidth="6xl">
            <BackLink to="/matrices" label="Matrices" />

            {/* Header */}
            <div className="mb-6">
                <div className="flex items-center gap-3">
                    <Grid3X3 className="size-5" />
                    <h1 className="text-2xl font-bold tracking-tight">{matrix.name}</h1>
                    <Badge
                        variant={matrix.layer === "invariant" ? "info" : "warning"}
                        size="sm"
                    >
                        {matrix.layer}
                    </Badge>
                </div>
                {matrix.description && (
                    <p className="text-muted-foreground mt-1 text-sm">{matrix.description}</p>
                )}
            </div>

            {/* Coverage summary */}
            {summary.total > 0 && (
                <div className="mb-6 flex flex-wrap items-center gap-4 rounded-lg border px-4 py-3 text-sm">
                    <span className="text-muted-foreground font-medium">Coverage:</span>
                    <span className="flex items-center gap-1.5">
                        <span className="inline-block size-2.5 rounded-full bg-emerald-500" />
                        {summary.specified} specified
                    </span>
                    <span className="flex items-center gap-1.5">
                        <span className="inline-block size-2.5 rounded-full bg-amber-500" />
                        {summary.unspecified} unspecified
                    </span>
                    <span className="flex items-center gap-1.5">
                        <span className="inline-block size-2.5 rounded-full bg-red-500" />
                        {summary.violated} violated
                    </span>
                    <span className="text-muted-foreground ml-auto tabular-nums">
                        {summary.total} total
                    </span>
                </div>
            )}

            {/* Grid */}
            <MatrixGrid
                dimensions={dimensions}
                rules={rules}
                cells={cells}
                onCellClick={handleCellClick}
                onToggleCell={handleToggleCell}
            />

            {/* Add controls */}
            <div className="mt-6 flex flex-col gap-4 sm:flex-row sm:gap-6">
                {/* Add dimension */}
                <form onSubmit={handleAddDimension} className="flex items-end gap-2">
                    <div className="space-y-1">
                        <label htmlFor="new-dim" className="text-xs font-medium text-muted-foreground">
                            Add Dimension
                        </label>
                        <Input
                            id="new-dim"
                            placeholder="Dimension name..."
                            value={newDimName}
                            onChange={e => setNewDimName(e.target.value)}
                            size="sm"
                        />
                    </div>
                    <Button
                        type="submit"
                        size="sm"
                        variant="outline"
                        disabled={!newDimName.trim() || addDimensionMutation.isPending}
                    >
                        <Plus className="size-3.5" />
                        Add
                    </Button>
                </form>

                {/* Add rule */}
                <form onSubmit={handleAddRule} className="flex items-end gap-2">
                    <div className="space-y-1">
                        <label htmlFor="new-rule" className="text-xs font-medium text-muted-foreground">
                            Add Rule
                        </label>
                        <Input
                            id="new-rule"
                            placeholder="Rule title..."
                            value={newRuleTitle}
                            onChange={e => setNewRuleTitle(e.target.value)}
                            size="sm"
                        />
                    </div>
                    <div className="space-y-1">
                        <label htmlFor="new-rule-cat" className="text-xs font-medium text-muted-foreground">
                            Category
                        </label>
                        <Input
                            id="new-rule-cat"
                            placeholder="Optional..."
                            value={newRuleCategory}
                            onChange={e => setNewRuleCategory(e.target.value)}
                            size="sm"
                        />
                    </div>
                    <Button
                        type="submit"
                        size="sm"
                        variant="outline"
                        disabled={!newRuleTitle.trim() || addRuleMutation.isPending}
                    >
                        <Plus className="size-3.5" />
                        Add
                    </Button>
                </form>
            </div>

            {/* Cell panel */}
            {selectedCell && (
                <CellPanel
                    matrixId={matrixId}
                    cellId={selectedCell.cellId}
                    ruleTitle={selectedCell.ruleTitle}
                    dimensionName={selectedCell.dimensionName}
                    onClose={() => setSelectedCell(null)}
                />
            )}
        </PageContainer>
    );
}
