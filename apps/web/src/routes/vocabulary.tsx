import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { BookOpen, ChevronDown, ChevronRight, Loader2, Pencil, Plus, Sparkles, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Empty, EmptyAction, EmptyDescription, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/vocabulary")({
    component: VocabularyPage,
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

type Category = "actor" | "action" | "target" | "outcome" | "state" | "modifier";

const CATEGORIES: Category[] = ["actor", "action", "target", "outcome", "state", "modifier"];
const EXPECTS_OPTIONS = ["actor", "action", "target", "outcome", "state"];

interface VocabEntry {
    id: string;
    word: string;
    category: string;
    expects: string[] | null;
    codebaseId: string;
    userId: string | null;
    createdAt: Date;
    updatedAt: Date;
}

interface SuggestedEntry {
    word: string;
    category: string;
    expects?: string[];
}

function categoryColor(category: string): string {
    switch (category) {
        case "actor":
            return "bg-blue-500/10 text-blue-600 border-blue-500/30 dark:text-blue-400";
        case "action":
            return "bg-amber-500/10 text-amber-600 border-amber-500/30 dark:text-amber-400";
        case "target":
            return "bg-green-500/10 text-green-600 border-green-500/30 dark:text-green-400";
        case "outcome":
            return "bg-purple-500/10 text-purple-600 border-purple-500/30 dark:text-purple-400";
        case "state":
            return "bg-cyan-500/10 text-cyan-600 border-cyan-500/30 dark:text-cyan-400";
        case "modifier":
            return "bg-gray-500/10 text-gray-600 border-gray-500/30 dark:text-gray-400";
        default:
            return "";
    }
}

function VocabularyPage() {
    const queryClient = useQueryClient();
    const { codebaseId } = useActiveCodebase();

    // Form state
    const [showForm, setShowForm] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);
    const [word, setWord] = useState("");
    const [category, setCategory] = useState<Category>("actor");
    const [expects, setExpects] = useState<string[]>([]);

    const [deleteTarget, setDeleteTarget] = useState<{ id: string; word: string } | null>(null);

    // Collapsible sections
    const [collapsedCategories, setCollapsedCategories] = useState<Set<string>>(new Set());

    // Suggestion state
    const [suggestions, setSuggestions] = useState<SuggestedEntry[] | null>(null);
    const [selectedSuggestions, setSelectedSuggestions] = useState<Set<number>>(new Set());

    const vocabQuery = useQuery({
        queryKey: ["vocabulary", codebaseId],
        queryFn: async () => {
            if (!codebaseId) return [];
            try {
                return unwrapEden(await api.api.vocabulary.get({ query: { codebaseId } })) as VocabEntry[];
            } catch {
                return [];
            }
        },
        enabled: !!codebaseId
    });

    const entries = Array.isArray(vocabQuery.data) ? vocabQuery.data : [];

    const createMutation = useMutation({
        mutationFn: async (body: { word: string; category: Category; expects?: string[]; codebaseId: string }) => {
            return unwrapEden(await api.api.vocabulary.post(body));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["vocabulary", codebaseId] });
            resetForm();
            toast.success("Entry added");
        },
        onError: () => {
            toast.error("Failed to add entry");
        }
    });

    const updateMutation = useMutation({
        mutationFn: async ({ id, body }: { id: string; body: { word?: string; category?: Category; expects?: string[] } }) => {
            return unwrapEden(await api.api.vocabulary({ id }).patch(body));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["vocabulary", codebaseId] });
            resetForm();
            toast.success("Entry updated");
        },
        onError: () => {
            toast.error("Failed to update entry");
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => {
            return unwrapEden(await api.api.vocabulary({ id }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["vocabulary", codebaseId] });
            toast.success("Entry deleted");
        },
        onError: () => {
            toast.error("Failed to delete entry");
        }
    });

    const suggestMutation = useMutation({
        mutationFn: async () => {
            if (!codebaseId) throw new Error("No codebase");
            return unwrapEden(await api.api.vocabulary.suggest.post({ codebaseId })) as SuggestedEntry[];
        },
        onSuccess: (data) => {
            const suggested = Array.isArray(data) ? data : [];
            setSuggestions(suggested);
            setSelectedSuggestions(new Set(suggested.map((_, i) => i)));
            if (suggested.length === 0) {
                toast.info("No suggestions found");
            }
        },
        onError: () => {
            toast.error("Failed to get suggestions");
        }
    });

    const bulkCreateMutation = useMutation({
        mutationFn: async (entriesToAdd: SuggestedEntry[]) => {
            if (!codebaseId) throw new Error("No codebase");
            return unwrapEden(
                await api.api.vocabulary.bulk.post({
                    entries: entriesToAdd.map(e => ({
                        word: e.word,
                        category: e.category as Category,
                        ...(e.expects ? { expects: e.expects } : {})
                    })),
                    codebaseId
                })
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["vocabulary", codebaseId] });
            setSuggestions(null);
            setSelectedSuggestions(new Set());
            toast.success("Entries added");
        },
        onError: () => {
            toast.error("Failed to add entries");
        }
    });

    function resetForm() {
        setShowForm(false);
        setEditingId(null);
        setWord("");
        setCategory("actor");
        setExpects([]);
    }

    function handleEdit(entry: VocabEntry) {
        setWord(entry.word);
        setCategory(entry.category as Category);
        setExpects(entry.expects ?? []);
        setEditingId(entry.id);
        setShowForm(true);
    }

    function handleDelete(id: string, entryWord: string) {
        setDeleteTarget({ id, word: entryWord });
    }

    function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        if (!word.trim() || !codebaseId) return;

        if (editingId) {
            updateMutation.mutate({
                id: editingId,
                body: {
                    word: word.trim().toLowerCase(),
                    category,
                    expects: expects.length > 0 ? expects : undefined
                }
            });
        } else {
            createMutation.mutate({
                word: word.trim().toLowerCase(),
                category,
                codebaseId,
                expects: expects.length > 0 ? expects : undefined
            });
        }
    }

    function toggleExpects(value: string) {
        setExpects(prev => (prev.includes(value) ? prev.filter(v => v !== value) : [...prev, value]));
    }

    function toggleCategory(cat: string) {
        setCollapsedCategories(prev => {
            const next = new Set(prev);
            if (next.has(cat)) {
                next.delete(cat);
            } else {
                next.add(cat);
            }
            return next;
        });
    }

    function toggleSuggestion(index: number) {
        setSelectedSuggestions(prev => {
            const next = new Set(prev);
            if (next.has(index)) {
                next.delete(index);
            } else {
                next.add(index);
            }
            return next;
        });
    }

    function handleAddSelected() {
        if (!suggestions) return;
        const selected = suggestions.filter((_, i) => selectedSuggestions.has(i));
        if (selected.length === 0) return;
        bulkCreateMutation.mutate(selected);
    }

    // Group entries by category
    const grouped = CATEGORIES.reduce(
        (acc, cat) => {
            acc[cat] = entries.filter(e => e.category === cat);
            return acc;
        },
        {} as Record<Category, VocabEntry[]>
    );

    if (!codebaseId) {
        return (
            <div className="container mx-auto max-w-5xl px-4 py-8">
                <div className="mb-6 flex items-center gap-2">
                    <BookOpen className="size-5" />
                    <h1 className="text-2xl font-bold tracking-tight">Vocabulary</h1>
                </div>
                <Card>
                    <CardPanel className="p-6">
                        <p className="text-muted-foreground text-sm">Select a codebase to manage vocabulary</p>
                    </CardPanel>
                </Card>
            </div>
        );
    }

    return (
        <div className="container mx-auto max-w-5xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <BookOpen className="size-5" />
                    <h1 className="text-2xl font-bold tracking-tight">Vocabulary</h1>
                    <Badge variant="secondary" className="ml-2">
                        {entries.length}
                    </Badge>
                </div>
                <div className="flex gap-2">
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => suggestMutation.mutate()}
                        disabled={suggestMutation.isPending}
                    >
                        {suggestMutation.isPending ? (
                            <Loader2 className="mr-1 size-4 animate-spin" />
                        ) : (
                            <Sparkles className="mr-1 size-4" />
                        )}
                        {suggestMutation.isPending ? "Suggesting..." : "Suggest from chunks"}
                    </Button>
                    {!showForm && (
                        <Button size="sm" onClick={() => setShowForm(true)}>
                            <Plus className="mr-1 size-4" />
                            Add Entry
                        </Button>
                    )}
                </div>
            </div>

            {/* Suggestions review panel */}
            {suggestions && suggestions.length > 0 && (
                <Card className="mb-6">
                    <CardPanel className="p-6">
                        <div className="mb-3 flex items-center justify-between">
                            <h2 className="text-sm font-medium">
                                Suggested Entries ({selectedSuggestions.size} of {suggestions.length} selected)
                            </h2>
                            <div className="flex gap-2">
                                <Button variant="ghost" size="sm" onClick={() => setSuggestions(null)}>
                                    Dismiss
                                </Button>
                                <Button
                                    size="sm"
                                    onClick={handleAddSelected}
                                    disabled={selectedSuggestions.size === 0 || bulkCreateMutation.isPending}
                                >
                                    {bulkCreateMutation.isPending ? "Adding..." : "Add selected"}
                                </Button>
                            </div>
                        </div>
                        <div className="space-y-1.5">
                            {suggestions.map((s, i) => (
                                <label
                                    key={i}
                                    className="hover:bg-muted flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 transition-colors"
                                >
                                    <input
                                        type="checkbox"
                                        checked={selectedSuggestions.has(i)}
                                        onChange={() => toggleSuggestion(i)}
                                        className="size-4"
                                    />
                                    <span className="font-medium">{s.word}</span>
                                    <Badge variant="outline" size="sm" className={categoryColor(s.category)}>
                                        {s.category}
                                    </Badge>
                                    {s.expects && s.expects.length > 0 && (
                                        <span className="text-muted-foreground text-xs">
                                            expects: {s.expects.join(", ")}
                                        </span>
                                    )}
                                </label>
                            ))}
                        </div>
                    </CardPanel>
                </Card>
            )}

            {/* Add / Edit form */}
            {showForm && (
                <Card className="mb-6">
                    <CardPanel className="p-6">
                        <form onSubmit={handleSubmit} className="space-y-3">
                            <h2 className="text-sm font-medium">{editingId ? "Edit Entry" : "Add Entry"}</h2>
                            <div className="flex flex-col gap-2 sm:flex-row">
                                <Input
                                    placeholder="Word or phrase"
                                    value={word}
                                    onChange={e => setWord(e.target.value)}
                                    required
                                    className="flex-1"
                                />
                                <select
                                    value={category}
                                    onChange={e => setCategory(e.target.value as Category)}
                                    className="bg-background focus:ring-ring w-36 rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                >
                                    {CATEGORIES.map(c => (
                                        <option key={c} value={c}>
                                            {c.charAt(0).toUpperCase() + c.slice(1)}
                                        </option>
                                    ))}
                                </select>
                            </div>
                            <div>
                                <label className="mb-1.5 block text-xs font-medium">Expects (optional)</label>
                                <div className="flex flex-wrap gap-2">
                                    {EXPECTS_OPTIONS.map(opt => (
                                        <label key={opt} className="flex cursor-pointer items-center gap-1.5 text-sm">
                                            <input
                                                type="checkbox"
                                                checked={expects.includes(opt)}
                                                onChange={() => toggleExpects(opt)}
                                                className="size-3.5"
                                            />
                                            {opt}
                                        </label>
                                    ))}
                                </div>
                            </div>
                            <div className="flex justify-end gap-2">
                                <Button type="button" variant="outline" size="sm" onClick={resetForm}>
                                    Cancel
                                </Button>
                                <Button
                                    type="submit"
                                    size="sm"
                                    disabled={
                                        createMutation.isPending ||
                                        updateMutation.isPending ||
                                        !word.trim()
                                    }
                                >
                                    {editingId
                                        ? updateMutation.isPending
                                            ? "Saving..."
                                            : "Save"
                                        : createMutation.isPending
                                          ? "Adding..."
                                          : "Add"}
                                </Button>
                            </div>
                        </form>
                    </CardPanel>
                </Card>
            )}

            {/* Grouped entries */}
            <Card>
                <CardPanel className="p-6">
                    {vocabQuery.isLoading ? (
                        <p className="text-muted-foreground text-sm">Loading...</p>
                    ) : entries.length === 0 ? (
                        <Empty>
                            <EmptyMedia variant="icon"><BookOpen className="h-10 w-10" /></EmptyMedia>
                            <EmptyTitle>No vocabulary</EmptyTitle>
                            <EmptyDescription>Define domain terms to standardize your knowledge base.</EmptyDescription>
                            <EmptyAction>
                                <Button onClick={() => setShowForm(true)}>Add Term</Button>
                            </EmptyAction>
                        </Empty>
                    ) : (
                        <div className="space-y-1">
                            {CATEGORIES.map(cat => {
                                const catEntries = grouped[cat];
                                if (catEntries.length === 0) return null;
                                const isCollapsed = collapsedCategories.has(cat);

                                return (
                                    <div key={cat}>
                                        <button
                                            type="button"
                                            onClick={() => toggleCategory(cat)}
                                            className="hover:bg-muted flex w-full items-center gap-2 rounded-md px-2 py-2 text-left transition-colors"
                                        >
                                            {isCollapsed ? (
                                                <ChevronRight className="size-4" />
                                            ) : (
                                                <ChevronDown className="size-4" />
                                            )}
                                            <Badge variant="outline" className={categoryColor(cat)}>
                                                {cat.charAt(0).toUpperCase() + cat.slice(1)}
                                            </Badge>
                                            <span className="text-muted-foreground text-xs">
                                                {catEntries.length} {catEntries.length === 1 ? "entry" : "entries"}
                                            </span>
                                        </button>

                                        {!isCollapsed && (
                                            <div className="ml-6 space-y-0.5">
                                                {catEntries.map(entry => (
                                                    <div
                                                        key={entry.id}
                                                        className="flex items-center justify-between rounded-md px-2 py-1.5"
                                                    >
                                                        <div className="flex items-center gap-2">
                                                            <span className="text-sm font-medium">{entry.word}</span>
                                                            {entry.expects && entry.expects.length > 0 && (
                                                                <span className="text-muted-foreground text-xs">
                                                                    expects:{" "}
                                                                    {entry.expects.map((ex, i) => (
                                                                        <Badge
                                                                            key={i}
                                                                            variant="outline"
                                                                            size="sm"
                                                                            className={`ml-0.5 text-[10px] ${categoryColor(ex)}`}
                                                                        >
                                                                            {ex}
                                                                        </Badge>
                                                                    ))}
                                                                </span>
                                                            )}
                                                        </div>
                                                        <div className="flex gap-1">
                                                            <Button
                                                                variant="ghost"
                                                                size="sm"
                                                                onClick={() => handleEdit(entry)}
                                                                title="Edit"
                                                            >
                                                                <Pencil className="size-3.5" />
                                                            </Button>
                                                            <Button
                                                                variant="ghost"
                                                                size="sm"
                                                                onClick={() => handleDelete(entry.id, entry.word)}
                                                                disabled={deleteMutation.isPending}
                                                                title="Delete"
                                                            >
                                                                <Trash2 className="size-3.5" />
                                                            </Button>
                                                        </div>
                                                    </div>
                                                ))}
                                            </div>
                                        )}
                                        <Separator className="my-1" />
                                    </div>
                                );
                            })}
                        </div>
                    )}
                </CardPanel>
            </Card>

            <ConfirmDialog
                open={deleteTarget !== null}
                onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}
                title="Delete vocabulary entry"
                description={deleteTarget ? `Delete vocabulary entry "${deleteTarget.word}"?` : ""}
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
        </div>
    );
}
