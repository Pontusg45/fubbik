import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
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

interface StepRow {
    keyword: Keyword;
    text: string;
}

interface StepError {
    step: number;
    error: string;
}

const KEYWORDS: Keyword[] = ["given", "when", "then", "and", "but"];
const PRIORITIES = [
    { value: "", label: "None" },
    { value: "must", label: "Must" },
    { value: "should", label: "Should" },
    { value: "could", label: "Could" },
    { value: "wont", label: "Won't" }
] as const;

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
    const [steps, setSteps] = useState<StepRow[]>([
        { keyword: "given", text: "" },
        { keyword: "when", text: "" },
        { keyword: "then", text: "" }
    ]);
    const [selectedChunkIds, setSelectedChunkIds] = useState<string[]>([]);
    const [chunkSearch, setChunkSearch] = useState("");
    const [errors, setErrors] = useState<Record<string, string>>({});
    const [stepErrors, setStepErrors] = useState<StepError[]>([]);

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
    }

    function removeStep(index: number) {
        setSteps(steps.filter((_, i) => i !== index));
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

    const createMutation = useMutation({
        mutationFn: async () => {
            const body: {
                title: string;
                description?: string;
                steps: Array<{ keyword: Keyword; text: string }>;
                priority?: "must" | "should" | "could" | "wont";
                codebaseId?: string;
            } = {
                title: title.trim(),
                steps: steps.map(s => ({ keyword: s.keyword, text: s.text.trim() }))
            };
            if (description.trim()) body.description = description.trim();
            if (priority) body.priority = priority as "must" | "should" | "could" | "wont";
            if (codebaseId) body.codebaseId = codebaseId;

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

                    <Separator />

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
                            {steps.map((step, i) => (
                                <div key={i} className="flex items-start gap-2">
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
                            ))}
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
