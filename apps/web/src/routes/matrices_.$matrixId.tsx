import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { ChevronDown, ChevronRight, Grid3X3, History, Plus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { BackLink } from "@/components/back-link";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogHeader, DialogPanel, DialogPopup, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { PageContainer, PageLoading } from "@/components/ui/page";
import { Textarea } from "@/components/ui/textarea";
import { CellPanel } from "@/features/matrices/cell-panel";
import { MatrixGrid, type Dimension, type Rule, type ViewCell } from "@/features/matrices/matrix-grid";
import { getUser } from "@/functions/get-user";
import { useApiQuery } from "@/hooks/use-api-query";
import { legacyApi } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/matrices_/$matrixId")({
    component: MatrixDetailPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
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
        verified: number;
        unspecified: number;
        violated: number;
        total: number;
    };
}

interface RuleHistoryEntry {
    id: string;
    ruleId: string;
    snapshot: {
        title: string;
        description: string | null;
        category: string | null;
        rationale: string | null;
        alternatives: string | null;
        consequences: string | null;
        counterexample: string | null;
    };
    changedBy: string | null;
    createdAt: string;
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

    // "Why" fields for new rule (collapsible/secondary)
    const [showWhyFields, setShowWhyFields] = useState(false);
    const [newRuleRationale, setNewRuleRationale] = useState("");
    const [newRuleAlternatives, setNewRuleAlternatives] = useState("");
    const [newRuleConsequences, setNewRuleConsequences] = useState("");
    const [newRuleCounterexample, setNewRuleCounterexample] = useState("");

    // Cell panel state
    const [selectedCell, setSelectedCell] = useState<SelectedCell | null>(null);

    // Rule history dialog state
    const [historyRule, setHistoryRule] = useState<{ id: string; title: string } | null>(null);

    const viewQuery = useApiQuery<MatrixView>({
        queryKey: ["matrix-view", matrixId],
        queryFn: () => legacyApi.api.matrices({ id: matrixId }).view.get()
    });

    const addDimensionMutation = useMutation({
        mutationFn: async (name: string) => unwrapEden(await legacyApi.api.matrices({ id: matrixId }).dimensions.post({ name })),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            setNewDimName("");
            toast.success("Dimension added");
        },
        onError: (err: unknown) => {
            toast.error(err instanceof Error ? err.message : "Failed to add dimension");
        }
    });

    const addRuleMutation = useMutation({
        mutationFn: async (body: {
            title: string;
            category?: string;
            rationale?: string;
            alternatives?: string;
            consequences?: string;
            counterexample?: string;
        }) => unwrapEden(await legacyApi.api.matrices({ id: matrixId }).rules.post(body)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            setNewRuleTitle("");
            setNewRuleCategory("");
            setNewRuleRationale("");
            setNewRuleAlternatives("");
            setNewRuleConsequences("");
            setNewRuleCounterexample("");
            setShowWhyFields(false);
            toast.success("Rule added");
        },
        onError: (err: unknown) => {
            toast.error(err instanceof Error ? err.message : "Failed to add rule");
        }
    });

    const toggleCellMutation = useMutation({
        mutationFn: async (body: { ruleId: string; dimensionId: string }) =>
            unwrapEden(await legacyApi.api.matrices({ id: matrixId }).cells.put(body)),
        onSuccess: data => {
            queryClient.invalidateQueries({ queryKey: ["matrix-view", matrixId] });
            if (data && typeof data === "object" && "action" in data) {
                const action = (data as { action: string }).action;
                toast.success(action === "created" ? "Cell created" : "Cell removed");
            }
        },
        onError: (err: unknown) => {
            toast.error(err instanceof Error ? err.message : "Failed to toggle cell");
        }
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
            ...(newRuleRationale.trim() ? { rationale: newRuleRationale.trim() } : {}),
            ...(newRuleAlternatives.trim() ? { alternatives: newRuleAlternatives.trim() } : {}),
            ...(newRuleConsequences.trim() ? { consequences: newRuleConsequences.trim() } : {}),
            ...(newRuleCounterexample.trim() ? { counterexample: newRuleCounterexample.trim() } : {})
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
            dimensionName: dim.name
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
                <div className="text-muted-foreground py-12 text-center">Failed to load matrix.</div>
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
                    <Badge variant={matrix.layer === "invariant" ? "info" : "warning"} size="sm">
                        {matrix.layer}
                    </Badge>
                </div>
                {matrix.description && <p className="text-muted-foreground mt-1 text-sm">{matrix.description}</p>}
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
                        <span className="inline-block size-2.5 rounded-full bg-green-600" />
                        {summary.verified} verified
                    </span>
                    <span className="flex items-center gap-1.5">
                        <span className="inline-block size-2.5 rounded-full bg-amber-500" />
                        {summary.unspecified} unspecified
                    </span>
                    <span className="flex items-center gap-1.5">
                        <span className="inline-block size-2.5 rounded-full bg-red-500" />
                        {summary.violated} violated
                    </span>
                    <span className="text-muted-foreground ml-auto tabular-nums">{summary.total} total</span>
                </div>
            )}

            {/* Grid */}
            <MatrixGrid dimensions={dimensions} rules={rules} cells={cells} onCellClick={handleCellClick} onToggleCell={handleToggleCell} />

            {/* Add controls */}
            <div className="mt-6 flex flex-col gap-4 sm:flex-row sm:gap-6">
                {/* Add dimension */}
                <form onSubmit={handleAddDimension} className="flex items-end gap-2">
                    <div className="space-y-1">
                        <label htmlFor="new-dim" className="text-muted-foreground text-xs font-medium">
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
                    <Button type="submit" size="sm" variant="outline" disabled={!newDimName.trim() || addDimensionMutation.isPending}>
                        <Plus className="size-3.5" />
                        Add
                    </Button>
                </form>

                {/* Add rule */}
                <form onSubmit={handleAddRule} className="flex-1 space-y-2">
                    <div className="flex items-end gap-2">
                        <div className="space-y-1">
                            <label htmlFor="new-rule" className="text-muted-foreground text-xs font-medium">
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
                            <label htmlFor="new-rule-cat" className="text-muted-foreground text-xs font-medium">
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
                        <Button type="submit" size="sm" variant="outline" disabled={!newRuleTitle.trim() || addRuleMutation.isPending}>
                            <Plus className="size-3.5" />
                            Add
                        </Button>
                    </div>

                    {/* Collapsible "why" fields */}
                    <button
                        type="button"
                        onClick={() => setShowWhyFields(v => !v)}
                        className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs font-medium transition-colors"
                    >
                        {showWhyFields ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
                        Decision context (optional)
                    </button>
                    {showWhyFields && (
                        <div className="grid gap-3 rounded-md border p-3 sm:grid-cols-2">
                            <div className="space-y-1">
                                <label htmlFor="new-rule-rationale" className="text-muted-foreground text-xs font-medium">
                                    Rationale
                                </label>
                                <Textarea
                                    id="new-rule-rationale"
                                    placeholder="Why does this rule exist?"
                                    value={newRuleRationale}
                                    onChange={e => setNewRuleRationale(e.target.value)}
                                    size="sm"
                                />
                            </div>
                            <div className="space-y-1">
                                <label htmlFor="new-rule-alternatives" className="text-muted-foreground text-xs font-medium">
                                    Alternatives
                                </label>
                                <Textarea
                                    id="new-rule-alternatives"
                                    placeholder="What else was considered?"
                                    value={newRuleAlternatives}
                                    onChange={e => setNewRuleAlternatives(e.target.value)}
                                    size="sm"
                                />
                            </div>
                            <div className="space-y-1">
                                <label htmlFor="new-rule-consequences" className="text-muted-foreground text-xs font-medium">
                                    Consequences
                                </label>
                                <Textarea
                                    id="new-rule-consequences"
                                    placeholder="What follows from this rule?"
                                    value={newRuleConsequences}
                                    onChange={e => setNewRuleConsequences(e.target.value)}
                                    size="sm"
                                />
                            </div>
                            <div className="space-y-1">
                                <label htmlFor="new-rule-counterexample" className="text-muted-foreground text-xs font-medium">
                                    Counterexample
                                </label>
                                <Textarea
                                    id="new-rule-counterexample"
                                    placeholder="A case that violates this rule"
                                    value={newRuleCounterexample}
                                    onChange={e => setNewRuleCounterexample(e.target.value)}
                                    size="sm"
                                />
                            </div>
                        </div>
                    )}
                </form>
            </div>

            {/* Rule history affordances */}
            {rules.length > 0 && (
                <div className="mt-6">
                    <h4 className="text-muted-foreground mb-2 text-xs font-semibold tracking-wide uppercase">Rule history</h4>
                    <ul className="divide-y rounded-lg border">
                        {rules.map(rule => (
                            <li key={rule.id} className="flex items-center gap-2 px-3 py-2">
                                <span className="line-clamp-1 flex-1 text-sm font-medium">{rule.title}</span>
                                {rule.category && (
                                    <Badge variant="secondary" size="sm">
                                        {rule.category}
                                    </Badge>
                                )}
                                <Button
                                    size="sm"
                                    variant="ghost"
                                    onClick={() => setHistoryRule({ id: rule.id, title: rule.title })}
                                    title="View rule history"
                                >
                                    <History className="size-3.5" />
                                    History
                                </Button>
                            </li>
                        ))}
                    </ul>
                </div>
            )}

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

            {/* Rule history dialog */}
            <Dialog open={historyRule !== null} onOpenChange={open => !open && setHistoryRule(null)}>
                {historyRule && (
                    <RuleHistoryDialog matrixId={matrixId} ruleId={historyRule.id} ruleTitle={historyRule.title} />
                )}
            </Dialog>
        </PageContainer>
    );
}

const HISTORY_FIELDS: { key: keyof RuleHistoryEntry["snapshot"]; label: string }[] = [
    { key: "title", label: "Title" },
    { key: "description", label: "Description" },
    { key: "category", label: "Category" },
    { key: "rationale", label: "Rationale" },
    { key: "alternatives", label: "Alternatives" },
    { key: "consequences", label: "Consequences" },
    { key: "counterexample", label: "Counterexample" }
];

function RuleHistoryDialog({ matrixId, ruleId, ruleTitle }: { matrixId: string; ruleId: string; ruleTitle: string }) {
    const historyQuery = useApiQuery<RuleHistoryEntry[]>({
        queryKey: ["matrix-rule-history", ruleId],
        queryFn: () => legacyApi.api.matrices({ id: matrixId }).rules({ ruleId }).history.get(),
        fallback: []
    });

    const history = Array.isArray(historyQuery.data) ? historyQuery.data : [];

    return (
        <DialogPopup className="max-w-2xl">
            <DialogHeader>
                <DialogTitle>History — {ruleTitle}</DialogTitle>
            </DialogHeader>
            <DialogPanel>
                {historyQuery.isLoading ? (
                    <p className="text-muted-foreground py-6 text-center text-sm">Loading history…</p>
                ) : history.length === 0 ? (
                    <p className="text-muted-foreground py-6 text-center text-sm">No history recorded for this rule.</p>
                ) : (
                    <ol className="space-y-4">
                        {history.map((entry, idx) => (
                            <li key={entry.id} className="rounded-lg border p-3">
                                <div className="mb-2 flex items-center justify-between gap-2">
                                    <span className="text-sm font-semibold">{entry.snapshot.title}</span>
                                    <span className="text-muted-foreground text-xs">
                                        {idx === 0 && (
                                            <Badge variant="info" size="sm" className="mr-2">
                                                latest
                                            </Badge>
                                        )}
                                        {new Date(entry.createdAt).toLocaleString()}
                                    </span>
                                </div>
                                <dl className="space-y-1.5 text-sm">
                                    {HISTORY_FIELDS.filter(f => f.key !== "title").map(field => {
                                        const value = entry.snapshot[field.key];
                                        if (!value) return null;
                                        return (
                                            <div key={field.key}>
                                                <dt className="text-muted-foreground text-xs font-medium tracking-wide uppercase">
                                                    {field.label}
                                                </dt>
                                                <dd className="whitespace-pre-wrap">{value}</dd>
                                            </div>
                                        );
                                    })}
                                </dl>
                            </li>
                        ))}
                    </ol>
                )}
            </DialogPanel>
        </DialogPopup>
    );
}
