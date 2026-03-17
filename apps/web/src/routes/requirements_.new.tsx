import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, ChevronDown, ChevronRight, Loader2, Sparkles } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { ChunkLinker } from "@/features/requirements/chunk-linker";
import { StepBuilder } from "@/features/requirements/step-builder";
import { validateSteps, type Keyword, type StepRow, type StepError } from "@/features/requirements/validation";
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

const PRIORITIES = [
    { value: "", label: "None" },
    { value: "must", label: "Must" },
    { value: "should", label: "Should" },
    { value: "could", label: "Could" },
    { value: "wont", label: "Won't" }
] as const;

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
                toast.success("Steps generated from description");
            }
        },
        onError: () => {
            toast.error("Failed to generate steps. Is Ollama running?");
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
                    <StepBuilder
                        steps={steps}
                        onStepsChange={setSteps}
                        codebaseId={codebaseId}
                        stepErrors={stepErrors}
                    />

                    <Separator />

                    {/* Linked chunks */}
                    <ChunkLinker
                        selectedChunkIds={selectedChunkIds}
                        onSelectedChunkIdsChange={setSelectedChunkIds}
                        codebaseId={codebaseId}
                    />

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
