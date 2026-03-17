import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Plus, Trash2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { Keyword, StepRow, StepError } from "@/features/requirements/validation";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

type VocabCategory = "actor" | "action" | "target" | "outcome" | "state" | "modifier";

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

interface StepBuilderProps {
    steps: StepRow[];
    onStepsChange: (steps: StepRow[]) => void;
    codebaseId: string | null | undefined;
    stepErrors: StepError[];
}

export function StepBuilder({ steps, onStepsChange, codebaseId, stepErrors }: StepBuilderProps) {
    const queryClient = useQueryClient();

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

    function updateStep(index: number, field: keyof StepRow, value: string) {
        onStepsChange(steps.map((s, i) => (i === index ? { ...s, [field]: value } : s)));
        if (field === "text") {
            triggerParse(index, value);
        }
    }

    function removeStep(index: number) {
        onStepsChange(steps.filter((_, i) => i !== index));
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
        onStepsChange([...steps, { keyword: "and", text: "" }]);
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

    return (
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
    );
}
