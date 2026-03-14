import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, ChevronDown, ChevronRight, Loader2, Plus, Sparkles, Trash2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/requirements_/new")({
    component: NewRequirement,
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            return { session: null };
        }
    }
});

type Keyword = "given" | "when" | "then" | "and" | "but";
type VocabCategory = "actor" | "action" | "target" | "outcome" | "state" | "modifier";

interface StepRow {
    keyword: Keyword;
    text: string;
}

interface StepError {
    step: number;
    error: string;
}

interface ParsedToken {
    text: string;
    category: string | null;
    position: { start: number; end: number };
}

interface VocabularyWarning {
    position: { start: number; end: number };
    type: "unknown_word" | "unexpected_category" | "expects_not_satisfied";
    word: string;
    message: string;
}

interface ParseResult {
    tokens: ParsedToken[];
    warnings: VocabularyWarning[];
}

const KEYWORDS: Keyword[] = ["given", "when", "then", "and", "but"];
const VOCAB_CATEGORIES: VocabCategory[] = ["actor", "action", "target", "outcome", "state", "modifier"];
const EXPECTS_OPTIONS = ["actor", "action", "target", "outcome", "state"];
const PRIORITIES = [
    { value: "", label: "None" },
    { value: "must", label: "Must" },
    { value: "should", label: "Should" },
    { value: "could", label: "Could" },
    { value: "wont", label: "Won't" }
] as const;

function tokenBadgeColor(category: string | null, hasWarning: boolean): string {
    if (hasWarning) return "bg-red-500/10 text-red-600 border-red-500/30 dark:text-red-400";
    if (category === null) return "bg-yellow-500/10 text-yellow-600 border-yellow-500/30 dark:text-yellow-400";
    if (category === "literal") return "bg-gray-500/10 text-gray-500 border-gray-500/30";
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
            return "bg-gray-500/10 text-gray-500 border-gray-500/30";
    }
}

function validateSteps(steps: StepRow[]): StepError[] {
    const errors: StepError[] = [];
    if (steps.length === 0) {
        errors.push({ step: 0, error: "Must have at least one step" });
        return errors;
    }

    const firstKeyword = steps[0]!.keyword;
    if (firstKeyword === "and" || firstKeyword === "but") {
        errors.push({ step: 0, error: "First step cannot be 'and' or 'but'" });
    } else if (firstKeyword !== "given") {
        errors.push({ step: 0, error: "First step must be 'given'" });
    }

    let phase: "given" | "when" | "then" = "given";
    for (let i = 0; i < steps.length; i++) {
        const { keyword } = steps[i]!;
        if (keyword === "and" || keyword === "but") continue;
        if (keyword === "given") {
            if (phase === "when" || phase === "then") {
                errors.push({ step: i, error: "Cannot use 'given' after 'when' or 'then'" });
            }
        } else if (keyword === "when") {
            if (phase === "then") {
                errors.push({ step: i, error: "Cannot use 'when' after 'then'" });
            } else {
                phase = "when";
            }
        } else if (keyword === "then") {
            if (phase === "given") {
                errors.push({ step: i, error: "'then' must come after 'when' phase" });
            } else {
                phase = "then";
            }
        }
    }

    if (!steps.some(s => s.keyword === "when")) {
        errors.push({ step: -1, error: "Must contain at least one 'when' step" });
    }
    if (!steps.some(s => s.keyword === "then")) {
        errors.push({ step: -1, error: "Must contain at least one 'then' step" });
    }

    return errors;
}

function NewRequirement() {
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const { codebaseId } = useActiveCodebase();

    const [title, setTitle] = useState("");
    const [description, setDescription] = useState("");
    const [priority, setPriority] = useState("");
    const [useCaseId, setUseCaseId] = useState("");
    const [steps, setSteps] = useState<StepRow[]>([
        { keyword: "given", text: "" },
        { keyword: "when", text: "" },
        { keyword: "then", text: "" }
    ]);
    const [selectedChunkIds, setSelectedChunkIds] = useState<string[]>([]);
    const [chunkSearch, setChunkSearch] = useState("");
    const [errors, setErrors] = useState<Record<string, string>>({});
    const [stepErrors, setStepErrors] = useState<StepError[]>([]);

    // AI description state
    const [aiDescription, setAiDescription] = useState("");
    const [aiExpanded, setAiExpanded] = useState(false);

    const generateStepsMutation = useMutation({
        mutationFn: async () => {
            const body: { description: string; codebaseId?: string } = {
                description: aiDescription.trim()
            };
            if (codebaseId) body.codebaseId = codebaseId;
            const result = unwrapEden(await api.api.ai["structure-requirement"].post(body)) as {
                steps: Array<{ keyword: Keyword; text: string }>;
            };
            return result;
        },
        onSuccess: data => {
            if (data.steps && data.steps.length > 0) {
                setSteps(data.steps.map(s => ({ keyword: s.keyword as Keyword, text: s.text })));
                setParseResults({});
                toast.success("Steps generated from description");
            }
        },
        onError: () => {
            toast.error("Failed to generate steps. Is Ollama running?");
        }
    });

    // Vocabulary parsing state
    const [parseResults, setParseResults] = useState<Record<number, ParseResult>>({});
    const [addingWordAtStep, setAddingWordAtStep] = useState<{ step: number; word: string } | null>(null);
    const [addCategory, setAddCategory] = useState<VocabCategory>("actor");
    const [addExpects, setAddExpects] = useState<string[]>([]);
    const debounceTimers = useRef<Record<number, ReturnType<typeof setTimeout>>>({});

    // Debounced parse function
    const triggerParse = useCallback(
        (stepIndex: number, text: string) => {
            if (!codebaseId || !text.trim()) {
                setParseResults(prev => {
                    const next = { ...prev };
                    delete next[stepIndex];
                    return next;
                });
                return;
            }

            // Clear existing timer
            if (debounceTimers.current[stepIndex]) {
                clearTimeout(debounceTimers.current[stepIndex]);
            }

            debounceTimers.current[stepIndex] = setTimeout(async () => {
                try {
                    const result = unwrapEden(
                        await api.api.vocabulary.parse.post({ text, codebaseId })
                    ) as ParseResult;
                    setParseResults(prev => ({ ...prev, [stepIndex]: result }));
                } catch {
                    // Silently ignore parse errors
                }
            }, 300);
        },
        [codebaseId]
    );

    // Cleanup timers on unmount
    useEffect(() => {
        return () => {
            for (const timer of Object.values(debounceTimers.current)) {
                clearTimeout(timer);
            }
        };
    }, []);

    const addWordMutation = useMutation({
        mutationFn: async (body: { word: string; category: VocabCategory; expects?: string[]; codebaseId: string }) => {
            return unwrapEden(await api.api.vocabulary.post(body));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["vocabulary", codebaseId] });
            toast.success("Word added to vocabulary");
            // Re-trigger parse for the step that had the word
            if (addingWordAtStep !== null) {
                const stepIndex = addingWordAtStep.step;
                const step = steps[stepIndex];
                if (step) {
                    triggerParse(stepIndex, step.text);
                }
            }
            setAddingWordAtStep(null);
            setAddCategory("actor");
            setAddExpects([]);
        },
        onError: () => {
            toast.error("Failed to add word");
        }
    });

    const useCasesQuery = useQuery({
        queryKey: ["use-cases", codebaseId],
        queryFn: async () => {
            try {
                const query: { codebaseId?: string } = {};
                if (codebaseId) query.codebaseId = codebaseId;
                const result = unwrapEden(await api.api["use-cases"].get({ query })) as
                    Array<{ id: string; name: string }>;
                return result ?? [];
            } catch {
                return [];
            }
        }
    });

    const allUseCases = useCasesQuery.data ?? [];

    const chunksQuery = useQuery({
        queryKey: ["chunks-for-linking", codebaseId],
        queryFn: async () => {
            try {
                const query: { codebaseId?: string; limit?: string } = { limit: "100" };
                if (codebaseId) query.codebaseId = codebaseId;
                const result = unwrapEden(await api.api.chunks.get({ query })) as {
                    chunks?: Array<{ id: string; title: string }>;
                } | null;
                return result?.chunks ?? [];
            } catch {
                return [];
            }
        }
    });

    const allChunks = chunksQuery.data ?? [];
    const filteredChunks = chunkSearch
        ? allChunks.filter(
              c => c.title.toLowerCase().includes(chunkSearch.toLowerCase()) && !selectedChunkIds.includes(c.id)
          )
        : allChunks.filter(c => !selectedChunkIds.includes(c.id));

    function updateStep(index: number, field: keyof StepRow, value: string) {
        setSteps(steps.map((s, i) => (i === index ? { ...s, [field]: value } : s)));
        if (field === "text") {
            triggerParse(index, value);
        }
    }

    function removeStep(index: number) {
        setSteps(steps.filter((_, i) => i !== index));
        setParseResults(prev => {
            const next: Record<number, ParseResult> = {};
            for (const [k, v] of Object.entries(prev)) {
                const ki = Number(k);
                if (ki === index) continue;
                next[ki > index ? ki - 1 : ki] = v;
            }
            return next;
        });
    }

    function addStep() {
        setSteps([...steps, { keyword: "and", text: "" }]);
    }

    function validate(): boolean {
        const e: Record<string, string> = {};
        if (!title.trim()) e.title = "Title is required";
        else if (title.length > 200) e.title = "Title must be 200 characters or less";

        const sErrors = validateSteps(steps);
        // Check for empty step text
        steps.forEach((s, i) => {
            if (!s.text.trim()) {
                sErrors.push({ step: i, error: "Step text is required" });
            }
        });

        setErrors(e);
        setStepErrors(sErrors);
        return Object.keys(e).length === 0 && sErrors.length === 0;
    }

    function stepHasError(index: number): boolean {
        return stepErrors.some(e => e.step === index);
    }

    function handleAddWord(stepIndex: number, word: string) {
        setAddingWordAtStep({ step: stepIndex, word });
        setAddCategory("actor");
        setAddExpects([]);
    }

    function submitAddWord() {
        if (!addingWordAtStep || !codebaseId) return;
        addWordMutation.mutate({
            word: addingWordAtStep.word.toLowerCase(),
            category: addCategory,
            codebaseId,
            ...(addExpects.length > 0 ? { expects: addExpects } : {})
        });
    }

    const createMutation = useMutation({
        mutationFn: async () => {
            const body: {
                title: string;
                description?: string;
                steps: Array<{ keyword: Keyword; text: string }>;
                priority?: "must" | "should" | "could" | "wont";
                codebaseId?: string;
                useCaseId?: string;
            } = {
                title: title.trim(),
                steps: steps.map(s => ({ keyword: s.keyword, text: s.text.trim() }))
            };
            if (description.trim()) body.description = description.trim();
            if (priority) body.priority = priority as "must" | "should" | "could" | "wont";
            if (codebaseId) body.codebaseId = codebaseId;
            if (useCaseId) body.useCaseId = useCaseId;

            const result = unwrapEden(await api.api.requirements.post(body)) as unknown as {
                requirement: { id: string };
                warnings: Array<{ step: number; type: string; reference: string }>;
            };

            // Link chunks if any selected
            if (selectedChunkIds.length > 0) {
                try {
                    await api.api.requirements({ id: result.requirement.id }).chunks.put({
                        chunkIds: selectedChunkIds
                    });
                } catch {
                    // non-critical
                }
            }

            return result;
        },
        onSuccess: (result: { requirement: { id: string }; warnings: Array<{ step: number; type: string; reference: string }> }) => {
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            queryClient.invalidateQueries({ queryKey: ["requirements-stats"] });

            if (result.warnings && result.warnings.length > 0) {
                toast.warning(`Created with ${result.warnings.length} warning(s)`);
            } else {
                toast.success("Requirement created");
            }

            navigate({
                to: "/requirements/$requirementId",
                params: { requirementId: result.requirement.id }
            });
        },
        onError: () => {
            toast.error("Failed to create requirement");
        }
    });

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <Button variant="ghost" size="sm" render={<Link to="/requirements" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
            </div>

            <h1 className="mb-6 text-2xl font-bold tracking-tight">New Requirement</h1>

            <Card>
                <CardPanel className="space-y-4 p-6">
                    {/* Title */}
                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Title</label>
                        <Input
                            value={title}
                            onChange={e => setTitle(e.target.value)}
                            placeholder="Enter requirement title..."
                            className={errors.title ? "border-red-500" : ""}
                        />
                        {errors.title && <p className="text-destructive mt-1 text-xs">{errors.title}</p>}
                    </div>

                    {/* Description */}
                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Description (optional)</label>
                        <textarea
                            value={description}
                            onChange={e => setDescription(e.target.value)}
                            placeholder="Describe the requirement..."
                            rows={3}
                            className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                        />
                    </div>

                    {/* Priority */}
                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Priority</label>
                        <select
                            value={priority}
                            onChange={e => setPriority(e.target.value)}
                            className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                        >
                            {PRIORITIES.map(p => (
                                <option key={p.value} value={p.value}>
                                    {p.label}
                                </option>
                            ))}
                        </select>
                    </div>

                    {/* Use Case */}
                    {allUseCases.length > 0 && (
                        <div>
                            <label className="mb-1.5 block text-sm font-medium">Use Case (optional)</label>
                            <select
                                value={useCaseId}
                                onChange={e => setUseCaseId(e.target.value)}
                                className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                            >
                                <option value="">(none)</option>
                                {allUseCases.map(uc => (
                                    <option key={uc.id} value={uc.id}>
                                        {uc.name}
                                    </option>
                                ))}
                            </select>
                        </div>
                    )}

                    <Separator />

                    {/* AI description section */}
                    <div>
                        <button
                            type="button"
                            onClick={() => setAiExpanded(!aiExpanded)}
                            className="mb-2 flex items-center gap-1.5 text-sm font-medium"
                        >
                            {aiExpanded ? (
                                <ChevronDown className="size-3.5" />
                            ) : (
                                <ChevronRight className="size-3.5" />
                            )}
                            <Sparkles className="size-3.5" />
                            Describe in plain English
                        </button>

                        {aiExpanded && (
                            <div className="bg-muted/50 mb-4 space-y-2 rounded-md border p-3">
                                <textarea
                                    value={aiDescription}
                                    onChange={e => setAiDescription(e.target.value)}
                                    placeholder="Describe what you want in plain English, e.g. 'When a user logs in with valid credentials, they should see the dashboard and their name in the header'"
                                    rows={3}
                                    className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                />
                                <Button
                                    type="button"
                                    variant="outline"
                                    size="sm"
                                    onClick={() => generateStepsMutation.mutate()}
                                    disabled={generateStepsMutation.isPending || !aiDescription.trim()}
                                >
                                    {generateStepsMutation.isPending ? (
                                        <>
                                            <Loader2 className="mr-1 size-3.5 animate-spin" />
                                            Generating...
                                        </>
                                    ) : (
                                        <>
                                            <Sparkles className="mr-1 size-3.5" />
                                            Generate steps
                                        </>
                                    )}
                                </Button>
                                <p className="text-muted-foreground text-xs">
                                    AI will convert your description into Given/When/Then steps. You can edit them after.
                                </p>
                            </div>
                        )}
                    </div>

                    {/* Steps builder */}
                    <div>
                        <div className="mb-2 flex items-center justify-between">
                            <label className="text-sm font-medium">Steps (Given / When / Then)</label>
                            <Button variant="ghost" size="sm" onClick={addStep}>
                                <Plus className="mr-1 size-3" />
                                Add step
                            </Button>
                        </div>

                        {/* Global step errors */}
                        {stepErrors
                            .filter(e => e.step === -1)
                            .map((e, i) => (
                                <p key={i} className="text-destructive mb-2 text-xs">
                                    {e.error}
                                </p>
                            ))}

                        <div className="space-y-2">
                            {steps.map((step, i) => {
                                const parseResult = parseResults[i];
                                const warningWords = new Set(
                                    parseResult?.warnings
                                        .filter(w => w.type === "unexpected_category")
                                        .map(w => w.word.toLowerCase()) ?? []
                                );

                                return (
                                    <div key={i}>
                                        <div className="flex items-start gap-2">
                                            <select
                                                value={step.keyword}
                                                onChange={e => updateStep(i, "keyword", e.target.value)}
                                                className="bg-background focus:ring-ring w-24 rounded-md border px-2 py-2 text-sm font-medium focus:ring-2 focus:outline-none"
                                            >
                                                {KEYWORDS.map(kw => (
                                                    <option key={kw} value={kw}>
                                                        {kw.charAt(0).toUpperCase() + kw.slice(1)}
                                                    </option>
                                                ))}
                                            </select>
                                            <Input
                                                value={step.text}
                                                onChange={e => updateStep(i, "text", e.target.value)}
                                                placeholder="Step description..."
                                                className={`flex-1 ${stepHasError(i) ? "border-red-500" : ""}`}
                                            />
                                            <Button
                                                variant="ghost"
                                                size="sm"
                                                onClick={() => removeStep(i)}
                                                disabled={steps.length <= 1}
                                            >
                                                <Trash2 className="size-3.5" />
                                            </Button>
                                        </div>

                                        {/* Vocabulary parse tokens */}
                                        {codebaseId && parseResult && parseResult.tokens.length > 0 && (
                                            <div className="ml-[6.5rem] mt-1 flex flex-wrap gap-1">
                                                {parseResult.tokens.map((token, ti) => {
                                                    const hasWarning = warningWords.has(token.text.toLowerCase());
                                                    const isUnknown = token.category === null;

                                                    return (
                                                        <span key={ti} className="inline-flex items-center gap-0.5">
                                                            <span
                                                                className={`inline-block rounded border px-1.5 py-0.5 text-[10px] font-medium ${tokenBadgeColor(token.category, hasWarning)}`}
                                                                title={
                                                                    hasWarning
                                                                        ? parseResult.warnings.find(
                                                                              w => w.word.toLowerCase() === token.text.toLowerCase()
                                                                          )?.message
                                                                        : isUnknown
                                                                          ? "Unknown word"
                                                                          : token.category ?? ""
                                                                }
                                                            >
                                                                {token.text}
                                                                {token.category && token.category !== "literal" && !isUnknown && (
                                                                    <span className="ml-0.5 opacity-60">{token.category}</span>
                                                                )}
                                                            </span>
                                                            {isUnknown && (
                                                                <button
                                                                    type="button"
                                                                    onClick={() => handleAddWord(i, token.text)}
                                                                    className="rounded bg-yellow-500/20 px-1 py-0.5 text-[9px] font-medium text-yellow-700 hover:bg-yellow-500/30 dark:text-yellow-400"
                                                                >
                                                                    Add?
                                                                </button>
                                                            )}
                                                        </span>
                                                    );
                                                })}
                                            </div>
                                        )}

                                        {/* Inline add word form */}
                                        {addingWordAtStep?.step === i && (
                                            <div className="ml-[6.5rem] mt-1.5 flex flex-wrap items-center gap-2 rounded-md border bg-yellow-500/5 p-2">
                                                <span className="text-xs font-medium">
                                                    Add &quot;{addingWordAtStep.word}&quot;:
                                                </span>
                                                <select
                                                    value={addCategory}
                                                    onChange={e => setAddCategory(e.target.value as VocabCategory)}
                                                    className="bg-background rounded border px-1.5 py-1 text-xs"
                                                >
                                                    {VOCAB_CATEGORIES.map(c => (
                                                        <option key={c} value={c}>
                                                            {c}
                                                        </option>
                                                    ))}
                                                </select>
                                                <div className="flex items-center gap-1.5">
                                                    {EXPECTS_OPTIONS.map(opt => (
                                                        <label key={opt} className="flex cursor-pointer items-center gap-0.5 text-[10px]">
                                                            <input
                                                                type="checkbox"
                                                                checked={addExpects.includes(opt)}
                                                                onChange={() =>
                                                                    setAddExpects(prev =>
                                                                        prev.includes(opt) ? prev.filter(v => v !== opt) : [...prev, opt]
                                                                    )
                                                                }
                                                                className="size-3"
                                                            />
                                                            {opt}
                                                        </label>
                                                    ))}
                                                </div>
                                                <Button
                                                    type="button"
                                                    size="sm"
                                                    variant="outline"
                                                    onClick={submitAddWord}
                                                    disabled={addWordMutation.isPending}
                                                    className="h-6 px-2 text-xs"
                                                >
                                                    {addWordMutation.isPending ? "..." : "Add"}
                                                </Button>
                                                <Button
                                                    type="button"
                                                    size="sm"
                                                    variant="ghost"
                                                    onClick={() => setAddingWordAtStep(null)}
                                                    className="h-6 px-1 text-xs"
                                                >
                                                    Cancel
                                                </Button>
                                            </div>
                                        )}
                                    </div>
                                );
                            })}
                        </div>

                        {/* Per-step errors */}
                        {stepErrors
                            .filter(e => e.step >= 0)
                            .map((e, i) => (
                                <p key={i} className="text-destructive mt-1 text-xs">
                                    Step {e.step + 1}: {e.error}
                                </p>
                            ))}
                    </div>

                    <Separator />

                    {/* Linked chunks */}
                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Linked Chunks (optional)</label>

                        {selectedChunkIds.length > 0 && (
                            <div className="mb-2 flex flex-wrap gap-2">
                                {selectedChunkIds.map(id => {
                                    const c = allChunks.find(ch => ch.id === id);
                                    return (
                                        <Badge
                                            key={id}
                                            variant="secondary"
                                            size="sm"
                                            className="cursor-pointer"
                                            onClick={() => setSelectedChunkIds(selectedChunkIds.filter(cid => cid !== id))}
                                        >
                                            {c?.title ?? id.slice(0, 8)} x
                                        </Badge>
                                    );
                                })}
                            </div>
                        )}

                        <Input
                            value={chunkSearch}
                            onChange={e => setChunkSearch(e.target.value)}
                            placeholder="Search chunks to link..."
                        />

                        {chunkSearch && filteredChunks.length > 0 && (
                            <div className="mt-1 max-h-40 overflow-y-auto rounded-md border">
                                {filteredChunks.slice(0, 10).map(c => (
                                    <button
                                        key={c.id}
                                        type="button"
                                        onClick={() => {
                                            setSelectedChunkIds([...selectedChunkIds, c.id]);
                                            setChunkSearch("");
                                        }}
                                        className="hover:bg-muted w-full px-3 py-1.5 text-left text-sm transition-colors"
                                    >
                                        {c.title}
                                    </button>
                                ))}
                            </div>
                        )}
                    </div>

                    <Separator />

                    <div className="flex justify-end gap-2">
                        <Button variant="outline" render={<Link to="/requirements" />}>
                            Cancel
                        </Button>
                        <Button
                            onClick={() => {
                                if (validate()) createMutation.mutate();
                            }}
                            disabled={createMutation.isPending}
                        >
                            {createMutation.isPending ? "Creating..." : "Create Requirement"}
                        </Button>
                    </div>
                </CardPanel>
            </Card>
        </div>
    );
}
