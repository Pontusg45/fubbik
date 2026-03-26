import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, ChevronDown, ChevronUp, Link2, Plus, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { PageContainer } from "@/components/ui/page";
import { Separator } from "@/components/ui/separator";
import { Textarea } from "@/components/ui/textarea";
import { DraftIndicator } from "@/features/chunks/draft-indicator";
import { loadDraft, useAutosave } from "@/features/chunks/use-autosave";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/plans/new")({
    component: NewPlan,
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            return { session: null };
        }
    }
});

interface StepRow {
    description: string;
    requirementId?: string;
}

function parseMarkdownPlan(md: string): {
    title: string;
    description: string;
    steps: StepRow[];
} {
    const lines = md.split("\n");
    let title = "";
    let description = "";
    const steps: StepRow[] = [];

    for (const line of lines) {
        const trimmed = line.trim();

        // Extract title from # H1
        if (!title && /^#\s+/.test(trimmed)) {
            title = trimmed.replace(/^#\s+/, "").trim();
            continue;
        }

        // Extract description from **Goal:**
        if (!description && /^\*\*Goal:\*\*/.test(trimmed)) {
            description = trimmed.replace(/^\*\*Goal:\*\*\s*/, "").trim();
            continue;
        }

        // Extract steps from - [ ] **Step N: Description** or - [ ] Description
        const checkboxMatch = trimmed.match(/^-\s*\[[ x]?\]\s*(?:\*\*(?:Step\s*\d+[:.]\s*)?)?(.+?)(?:\*\*)?$/i);
        if (checkboxMatch) {
            steps.push({ description: checkboxMatch[1]!.replace(/\*\*$/, "").trim() });
            continue;
        }

        // Extract task groups from ## Task N: Name
        const taskMatch = trimmed.match(/^##\s+(?:Task|Step)\s*\d+[:.]\s*(.+)/i);
        if (taskMatch) {
            steps.push({ description: taskMatch[1]!.trim() });
            continue;
        }

        // Simple list items as steps
        const listMatch = trimmed.match(/^[-*]\s+(.+)/);
        if (listMatch && !trimmed.startsWith("**Goal:")) {
            steps.push({ description: listMatch[1]!.trim() });
        }
    }

    return { title, description, steps: steps.length > 0 ? steps : [{ description: "" }] };
}

function NewPlan() {
    const navigate = useNavigate();
    const { codebaseId } = useActiveCodebase();
    const [title, setTitle] = useState("");
    const [description, setDescription] = useState("");
    const [steps, setSteps] = useState<StepRow[]>([{ description: "" }]);
    const [mode, setMode] = useState<"builder" | "markdown">("builder");
    const [markdownInput, setMarkdownInput] = useState("");
    const [showBulkEntry, setShowBulkEntry] = useState(false);
    const [bulkText, setBulkText] = useState("");
    const [linkingStepIndex, setLinkingStepIndex] = useState<number | null>(null);
    const linkDropdownRef = useRef<HTMLDivElement>(null);

    // Autosave
    const formState = useMemo(() => ({ title, description, steps }), [title, description, steps]);
    const { clearDraft, lastSaved } = useAutosave("plan-draft-new", formState);

    // Restore draft on mount
    useEffect(() => {
        const draft = loadDraft<{ title: string; description: string; steps: Array<{ description: string; requirementId?: string }> }>("plan-draft-new");
        if (draft && (draft.title || draft.steps?.some(s => s.description))) {
            setTitle(draft.title ?? "");
            setDescription(draft.description ?? "");
            if (draft.steps?.length) setSteps(draft.steps);
            toast.info("Restored unsaved plan draft");
        }
    }, []);

    // Close requirement dropdown when clicking outside
    useEffect(() => {
        if (linkingStepIndex === null) return;
        function handleClick(e: MouseEvent) {
            if (linkDropdownRef.current && !linkDropdownRef.current.contains(e.target as Node)) {
                setLinkingStepIndex(null);
            }
        }
        document.addEventListener("mousedown", handleClick);
        return () => document.removeEventListener("mousedown", handleClick);
    }, [linkingStepIndex]);

    // Templates
    const templatesQuery = useQuery({
        queryKey: ["plan-templates"],
        queryFn: async () => {
            const result = unwrapEden(await api.api.plans.templates.get());
            return (result as any)?.templates ?? [];
        },
        staleTime: 60_000
    });

    // Requirements for linking
    const reqsQuery = useQuery({
        queryKey: ["requirements-for-linking"],
        queryFn: async () => unwrapEden(await api.api.requirements.get({ query: {} })),
        staleTime: 30_000
    });

    const createMutation = useMutation({
        mutationFn: async () => {
            const validSteps = steps
                .filter(s => s.description.trim())
                .map((s, i) => ({
                    description: s.description.trim(),
                    order: i,
                    ...(s.requirementId ? { requirementId: s.requirementId } : {})
                }));

            return unwrapEden(
                await api.api.plans.post({
                    title: title.trim(),
                    ...(description.trim() ? { description: description.trim() } : {}),
                    ...(validSteps.length > 0 ? { steps: validSteps } : {}),
                    ...(codebaseId ? { codebaseId } : {})
                })
            );
        },
        onSuccess: (data) => {
            clearDraft();
            toast.success("Plan created");
            const planId = (data as Record<string, unknown>)?.id as string | undefined;
            if (planId) {
                navigate({ to: "/plans/$planId", params: { planId } });
            } else {
                navigate({ to: "/plans" });
            }
        },
        onError: () => {
            toast.error("Failed to create plan");
        }
    });

    function addStepAfter(index: number) {
        const next = [...steps];
        next.splice(index + 1, 0, { description: "" });
        setSteps(next);
        // Focus new step after render
        setTimeout(() => {
            const inputs = document.querySelectorAll<HTMLInputElement>("[data-step-input]");
            inputs[index + 1]?.focus();
        }, 0);
    }

    function removeStep(index: number) {
        if (steps.length <= 1) return;
        setSteps(steps.filter((_, i) => i !== index));
        // Focus previous step
        setTimeout(() => {
            const inputs = document.querySelectorAll<HTMLInputElement>("[data-step-input]");
            const focusIdx = Math.max(0, index - 1);
            inputs[focusIdx]?.focus();
        }, 0);
    }

    function updateStep(index: number, value: string) {
        setSteps(steps.map((s, i) => (i === index ? { ...s, description: value } : s)));
    }

    function moveStep(index: number, direction: "up" | "down") {
        const targetIndex = direction === "up" ? index - 1 : index + 1;
        if (targetIndex < 0 || targetIndex >= steps.length) return;
        const next = [...steps];
        const temp = next[index]!;
        next[index] = next[targetIndex]!;
        next[targetIndex] = temp;
        setSteps(next);
        // Focus the moved step at its new position
        setTimeout(() => {
            const inputs = document.querySelectorAll<HTMLInputElement>("[data-step-input]");
            inputs[targetIndex]?.focus();
        }, 0);
    }

    function linkRequirement(stepIndex: number, requirementId: string | undefined) {
        setSteps(steps.map((s, i) => (i === stepIndex ? { ...s, requirementId } : s)));
        setLinkingStepIndex(null);
    }

    function handleBulkAdd() {
        const newSteps = bulkText
            .split("\n")
            .map(line => line.trim())
            .filter(line => line.length > 0)
            .map(line => ({ description: line }));
        if (newSteps.length === 0) return;
        // If only one empty step exists, replace it; otherwise append
        const hasContent = steps.some(s => s.description.trim());
        if (!hasContent && steps.length === 1) {
            setSteps(newSteps);
        } else {
            setSteps([...steps, ...newSteps]);
        }
        setBulkText("");
        setShowBulkEntry(false);
        toast.success(`Added ${newSteps.length} step${newSteps.length > 1 ? "s" : ""}`);
    }

    function handleParseMarkdown() {
        const parsed = parseMarkdownPlan(markdownInput);
        if (parsed.title) setTitle(parsed.title);
        if (parsed.description) setDescription(parsed.description);
        setSteps(parsed.steps);
        setMode("builder");
        setMarkdownInput("");
        toast.success(`Parsed: ${parsed.steps.length} step${parsed.steps.length !== 1 ? "s" : ""} found`);
    }

    function handleStepKeyDown(e: React.KeyboardEvent<HTMLInputElement>, index: number) {
        if (e.key === "Enter") {
            e.preventDefault();
            addStepAfter(index);
        } else if (e.key === "Backspace" && !steps[index]?.description && steps.length > 1) {
            e.preventDefault();
            removeStep(index);
        } else if (e.key === "ArrowUp" && e.altKey) {
            e.preventDefault();
            moveStep(index, "up");
        } else if (e.key === "ArrowDown" && e.altKey) {
            e.preventDefault();
            moveStep(index, "down");
        }
    }

    function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        if (!title.trim()) return;
        createMutation.mutate();
    }

    const validStepCount = steps.filter(s => s.description.trim()).length;
    const hasAnyFilledStep = steps.some(s => s.description.trim());
    const requirements = (reqsQuery.data as any)?.requirements ?? (Array.isArray(reqsQuery.data) ? reqsQuery.data : []);

    return (
        <PageContainer>
            <div className="mb-6">
                <Button variant="ghost" size="sm" render={<Link to="/plans" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
            </div>

            <h1 className="mb-6 text-2xl font-bold tracking-tight">New Plan</h1>

            <Card>
                <CardPanel className="p-6">
                    <form onSubmit={handleSubmit} className="space-y-4">
                        {(templatesQuery.data ?? []).length > 0 && (
                            <div className="mb-4">
                                <label className="mb-1.5 block text-sm font-medium">Start from template</label>
                                <div className="flex flex-wrap gap-2">
                                    {(templatesQuery.data ?? []).map((tmpl: any) => (
                                        <Button
                                            key={tmpl.key}
                                            type="button"
                                            variant="outline"
                                            size="sm"
                                            onClick={() => {
                                                setTitle(tmpl.title);
                                                setDescription(tmpl.description);
                                                if (tmpl.steps?.length) {
                                                    setSteps(tmpl.steps.map((s: string) => ({ description: s })));
                                                }
                                            }}
                                        >
                                            {tmpl.title}
                                        </Button>
                                    ))}
                                </div>
                            </div>
                        )}

                        {/* Mode toggle */}
                        <div className="flex gap-1 mb-4">
                            <Button type="button" variant={mode === "builder" ? "default" : "outline"} size="sm" onClick={() => setMode("builder")}>
                                Step Builder
                            </Button>
                            <Button type="button" variant={mode === "markdown" ? "default" : "outline"} size="sm" onClick={() => setMode("markdown")}>
                                Paste Markdown
                            </Button>
                        </div>

                        {mode === "markdown" ? (
                            <div className="space-y-3">
                                <Textarea
                                    value={markdownInput}
                                    onChange={e => setMarkdownInput((e.target as HTMLTextAreaElement).value)}
                                    placeholder={"Paste your plan markdown here...\n\n# Plan Title\n**Goal:** Description\n- [ ] Step 1\n- [ ] Step 2"}
                                    rows={12}
                                    className="font-mono text-sm"
                                />
                                <Button
                                    type="button"
                                    onClick={handleParseMarkdown}
                                    disabled={!markdownInput.trim()}
                                >
                                    Parse & Switch to Builder
                                </Button>
                            </div>
                        ) : (
                            <>
                                <div>
                                    <label className="mb-1.5 block text-sm font-medium">Title</label>
                                    <Input
                                        type="text"
                                        value={title}
                                        onChange={e => setTitle((e.target as HTMLInputElement).value)}
                                        placeholder="Enter a plan title..."
                                        required
                                    />
                                </div>

                                <div>
                                    <label className="mb-1.5 block text-sm font-medium">Description</label>
                                    <Textarea
                                        value={description}
                                        onChange={e => setDescription((e.target as HTMLTextAreaElement).value)}
                                        placeholder="Describe what this plan covers (optional)..."
                                        rows={3}
                                    />
                                </div>

                                <Separator />

                                <div>
                                    <label className="mb-2 block text-sm font-medium">
                                        Steps{" "}
                                        <span className="text-muted-foreground text-xs">
                                            ({validStepCount} valid)
                                        </span>
                                    </label>
                                    <div className="space-y-2">
                                        {steps.map((step, i) => (
                                            <div key={i} className="flex items-center gap-1">
                                                <span className="text-muted-foreground w-6 text-right text-xs font-mono">
                                                    {i + 1}.
                                                </span>
                                                <div className="flex-1 relative">
                                                    <Input
                                                        type="text"
                                                        data-step-input
                                                        value={step.description}
                                                        onChange={e => updateStep(i, (e.target as HTMLInputElement).value)}
                                                        placeholder="Step description..."
                                                        onKeyDown={e => handleStepKeyDown(e, i)}
                                                        className={!step.description.trim() && hasAnyFilledStep ? "border-l-2 border-l-yellow-400" : ""}
                                                    />
                                                    {step.requirementId && (
                                                        <span className="absolute right-2 top-1/2 -translate-y-1/2 bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300 text-[10px] font-medium px-1.5 py-0.5 rounded">
                                                            req
                                                        </span>
                                                    )}
                                                </div>
                                                {/* Reorder buttons */}
                                                <Button
                                                    type="button"
                                                    variant="ghost"
                                                    size="sm"
                                                    onClick={() => moveStep(i, "up")}
                                                    disabled={i === 0}
                                                    title="Move step up"
                                                    className="size-7 p-0"
                                                >
                                                    <ChevronUp className="size-3.5" />
                                                </Button>
                                                <Button
                                                    type="button"
                                                    variant="ghost"
                                                    size="sm"
                                                    onClick={() => moveStep(i, "down")}
                                                    disabled={i === steps.length - 1}
                                                    title="Move step down"
                                                    className="size-7 p-0"
                                                >
                                                    <ChevronDown className="size-3.5" />
                                                </Button>
                                                {/* Link requirement */}
                                                <div className="relative">
                                                    <Button
                                                        type="button"
                                                        variant="ghost"
                                                        size="sm"
                                                        onClick={() => setLinkingStepIndex(linkingStepIndex === i ? null : i)}
                                                        title={step.requirementId ? "Change linked requirement" : "Link requirement"}
                                                        className={`size-7 p-0 ${step.requirementId ? "text-blue-600" : ""}`}
                                                    >
                                                        <Link2 className="size-3.5" />
                                                    </Button>
                                                    {linkingStepIndex === i && (
                                                        <div
                                                            ref={linkDropdownRef}
                                                            className="absolute right-0 top-full z-50 mt-1 w-64 max-h-48 overflow-y-auto rounded-md border bg-popover p-1 shadow-md"
                                                        >
                                                            {step.requirementId && (
                                                                <button
                                                                    type="button"
                                                                    className="w-full text-left px-2 py-1.5 text-xs rounded hover:bg-accent text-muted-foreground"
                                                                    onClick={() => linkRequirement(i, undefined)}
                                                                >
                                                                    Remove link
                                                                </button>
                                                            )}
                                                            {requirements.length === 0 && (
                                                                <div className="px-2 py-1.5 text-xs text-muted-foreground">
                                                                    No requirements found
                                                                </div>
                                                            )}
                                                            {requirements.map((req: any) => (
                                                                <button
                                                                    key={req.id}
                                                                    type="button"
                                                                    className={`w-full text-left px-2 py-1.5 text-xs rounded hover:bg-accent ${step.requirementId === req.id ? "bg-accent font-medium" : ""}`}
                                                                    onClick={() => linkRequirement(i, req.id)}
                                                                >
                                                                    {req.title}
                                                                </button>
                                                            ))}
                                                        </div>
                                                    )}
                                                </div>
                                                <Button
                                                    type="button"
                                                    variant="ghost"
                                                    size="sm"
                                                    onClick={() => addStepAfter(i)}
                                                    title="Add step below"
                                                    className="size-7 p-0"
                                                >
                                                    <Plus className="size-3.5" />
                                                </Button>
                                                <Button
                                                    type="button"
                                                    variant="ghost"
                                                    size="sm"
                                                    onClick={() => removeStep(i)}
                                                    disabled={steps.length <= 1}
                                                    title="Remove step"
                                                    className="size-7 p-0"
                                                >
                                                    <Trash2 className="size-3.5" />
                                                </Button>
                                            </div>
                                        ))}
                                    </div>

                                    <p className="text-muted-foreground text-xs mt-1">
                                        Enter: add step &middot; Backspace on empty: remove &middot; Alt+&uarr;/&darr;: reorder
                                    </p>

                                    {/* Bulk step entry */}
                                    <div className="mt-3">
                                        {!showBulkEntry ? (
                                            <Button
                                                type="button"
                                                variant="ghost"
                                                size="sm"
                                                onClick={() => setShowBulkEntry(true)}
                                            >
                                                Paste multiple steps
                                            </Button>
                                        ) : (
                                            <div className="space-y-2">
                                                <Textarea
                                                    value={bulkText}
                                                    onChange={e => setBulkText((e.target as HTMLTextAreaElement).value)}
                                                    placeholder="Paste one step per line..."
                                                    rows={5}
                                                    className="text-sm"
                                                />
                                                <div className="flex gap-2">
                                                    <Button
                                                        type="button"
                                                        size="sm"
                                                        onClick={handleBulkAdd}
                                                        disabled={!bulkText.trim()}
                                                    >
                                                        Add {bulkText.split("\n").filter(l => l.trim()).length} step{bulkText.split("\n").filter(l => l.trim()).length !== 1 ? "s" : ""}
                                                    </Button>
                                                    <Button
                                                        type="button"
                                                        variant="ghost"
                                                        size="sm"
                                                        onClick={() => {
                                                            setShowBulkEntry(false);
                                                            setBulkText("");
                                                        }}
                                                    >
                                                        Cancel
                                                    </Button>
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                </div>

                                <Separator />

                                <div className="flex items-center justify-end gap-2">
                                    <DraftIndicator lastSaved={lastSaved} />
                                    <Button type="button" variant="outline" render={<Link to="/plans" />}>
                                        Cancel
                                    </Button>
                                    <Button
                                        type="submit"
                                        disabled={createMutation.isPending || !title.trim()}
                                    >
                                        {createMutation.isPending ? "Creating..." : "Create Plan"}
                                    </Button>
                                </div>
                            </>
                        )}
                    </form>
                </CardPanel>
            </Card>
        </PageContainer>
    );
}
