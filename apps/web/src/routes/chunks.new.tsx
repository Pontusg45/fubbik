import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, ChevronDown, FileText, Loader2, Plus, Sparkles, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { chunkTemplates } from "@/features/chunks/templates";
import { MarkdownEditor } from "@/features/editor/markdown-editor";
import { getUser } from "@/functions/get-user";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/chunks/new")({
    component: NewChunk,
    beforeLoad: async () => {
        try {
            const session = await getUser();
            return { session };
        } catch {
            return { session: null };
        }
    }
});

interface ApplyToRow {
    pattern: string;
    note: string;
}

interface FileRefRow {
    path: string;
    anchor: string;
    relation: "documents" | "configures" | "tests" | "implements";
}

function NewChunk() {
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const [title, setTitle] = useState("");
    const [content, setContent] = useState("");
    const [type, setType] = useState("note");
    const [tagInput, setTagInput] = useState("");
    const [tags, setTags] = useState<string[]>([]);
    const [errors, setErrors] = useState<Record<string, string>>({});
    const [aiPrompt, setAiPrompt] = useState("");
    const debouncedTitle = useDebouncedValue(title, 500);

    // New fields
    const [appliesTo, setAppliesTo] = useState<ApplyToRow[]>([]);
    const [fileRefs, setFileRefs] = useState<FileRefRow[]>([]);
    const [showDecisionContext, setShowDecisionContext] = useState(false);
    const [rationale, setRationale] = useState("");
    const [alternativesInput, setAlternativesInput] = useState("");
    const [consequences, setConsequences] = useState("");
    const [selectedTemplateId, setSelectedTemplateId] = useState("");

    const templatesQuery = useQuery({
        queryKey: ["templates"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.templates.get()) as Array<{
                    id: string;
                    name: string;
                    description: string | null;
                    type: string;
                    content: string;
                    isBuiltIn: boolean;
                }>;
            } catch {
                return [];
            }
        }
    });

    const serverTemplates = Array.isArray(templatesQuery.data) ? templatesQuery.data : [];

    const duplicateQuery = useQuery({
        queryKey: ["duplicate-check", debouncedTitle],
        queryFn: async () => {
            if (!debouncedTitle.trim() || debouncedTitle.length < 3) return [];
            try {
                const result = unwrapEden(await api.api.chunks.get({ query: { search: debouncedTitle, limit: "3" } })) as {
                    chunks?: { id: string; title: string }[];
                } | null;
                return result?.chunks?.filter(c => c.title.toLowerCase() !== debouncedTitle.toLowerCase()).slice(0, 3) ?? [];
            } catch {
                return [];
            }
        },
        enabled: debouncedTitle.length >= 3
    });

    const duplicates = duplicateQuery.data ?? [];

    const generateMutation = useMutation({
        mutationFn: async () => {
            const { data, error } = await api.api.ai.generate.post({ prompt: aiPrompt });
            if (error) throw new Error("Failed to generate chunk");
            return data as { title: string; content: string; type: string; tags: string[] };
        },
        onSuccess: data => {
            setTitle(data.title);
            setContent(data.content);
            if (data.type) setType(data.type);
            if (data.tags?.length) setTags(data.tags);
            setAiPrompt("");
            toast.success("Chunk generated from AI");
        },
        onError: () => {
            toast.error("Failed to generate chunk");
        }
    });

    function validate() {
        const e: Record<string, string> = {};
        if (!title.trim()) e.title = "Title is required";
        else if (title.length > 200) e.title = "Title must be 200 characters or less";
        if (content.length > 50000) e.content = "Content must be 50,000 characters or less";
        setErrors(e);
        return Object.keys(e).length === 0;
    }

    const createMutation = useMutation({
        mutationFn: async () => {
            const alternatives = alternativesInput
                .split(",")
                .map(s => s.trim())
                .filter(Boolean);
            const chunk = unwrapEden(
                await api.api.chunks.post({
                    title,
                    content,
                    type,
                    tags,
                    ...(rationale ? { rationale } : {}),
                    ...(alternatives.length > 0 ? { alternatives } : {}),
                    ...(consequences ? { consequences } : {})
                })
            );

            const chunkId = (chunk as Record<string, unknown>)?.id as string | undefined;
            if (!chunkId) return chunk;

            // Set applies-to after chunk creation
            const validAppliesTo = appliesTo.filter(a => a.pattern.trim());
            if (validAppliesTo.length > 0) {
                try {
                    await api.api.chunks({ id: chunkId })["applies-to"].put(
                        validAppliesTo.map(a => ({
                            pattern: a.pattern.trim(),
                            ...(a.note.trim() ? { note: a.note.trim() } : {})
                        }))
                    );
                } catch {
                    // non-critical
                }
            }

            // Set file refs after chunk creation
            const validFileRefs = fileRefs.filter(f => f.path.trim());
            if (validFileRefs.length > 0) {
                try {
                    await api.api.chunks({ id: chunkId })["file-refs"].put(
                        validFileRefs.map(f => ({
                            path: f.path.trim(),
                            ...(f.anchor.trim() ? { anchor: f.anchor.trim() } : {}),
                            relation: f.relation
                        }))
                    );
                } catch {
                    // non-critical
                }
            }

            return chunk;
        },
        onSuccess: data => {
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            toast.success("Chunk created");
            if (data && typeof data === "object" && "id" in (data as Record<string, unknown>)) {
                navigate({ to: "/chunks/$chunkId", params: { chunkId: (data as Record<string, unknown>).id as string } });
            }
        },
        onError: () => {
            toast.error("Failed to create chunk");
        }
    });

    const addTag = () => {
        const tag = tagInput.trim().toLowerCase();
        if (tag && !tags.includes(tag)) {
            setTags([...tags, tag]);
        }
        setTagInput("");
    };

    function handleTemplateSelect(templateId: string) {
        setSelectedTemplateId(templateId);
        if (!templateId) return;

        // Check server templates first
        const serverTmpl = serverTemplates.find(t => t.id === templateId);
        if (serverTmpl) {
            setContent(serverTmpl.content);
            setType(serverTmpl.type);
            return;
        }

        // Check local templates
        const localTmpl = chunkTemplates.find(t => t.name === templateId);
        if (localTmpl) {
            setContent(localTmpl.content);
            setType(localTmpl.type);
            setTags(localTmpl.tags);
        }
    }

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
            </div>

            <h1 className="mb-6 text-2xl font-bold tracking-tight">New Chunk</h1>

            <Card className="mb-6">
                <CardPanel className="p-6">
                    <div className="mb-3 flex items-center gap-2">
                        <Sparkles className="size-4" />
                        <span className="text-sm font-medium">AI Generate</span>
                    </div>
                    <div className="flex gap-2">
                        <input
                            type="text"
                            value={aiPrompt}
                            onChange={e => setAiPrompt(e.target.value)}
                            onKeyDown={e => {
                                if (e.key === "Enter" && aiPrompt.trim()) {
                                    e.preventDefault();
                                    generateMutation.mutate();
                                }
                            }}
                            placeholder="Describe what you want..."
                            className="bg-background focus:ring-ring flex-1 rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                        />
                        <Button
                            variant="outline"
                            onClick={() => generateMutation.mutate()}
                            disabled={generateMutation.isPending || !aiPrompt.trim()}
                        >
                            {generateMutation.isPending ? <Loader2 className="size-4 animate-spin" /> : <Sparkles className="size-4" />}
                            {generateMutation.isPending ? "Generating..." : "Generate"}
                        </Button>
                    </div>
                </CardPanel>
            </Card>

            <Card className="mb-6">
                <CardPanel className="p-6">
                    <div className="mb-3 flex items-center gap-2">
                        <FileText className="size-4" />
                        <span className="text-sm font-medium">Start from Template</span>
                    </div>
                    <select
                        value={selectedTemplateId}
                        onChange={e => handleTemplateSelect(e.target.value)}
                        className="bg-background focus:ring-ring mb-3 w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                    >
                        <option value="">(none)</option>
                        {serverTemplates.length > 0 && (
                            <optgroup label="Custom Templates">
                                {serverTemplates.map(t => (
                                    <option key={t.id} value={t.id}>
                                        {t.name} {t.isBuiltIn ? "(built-in)" : ""}
                                    </option>
                                ))}
                            </optgroup>
                        )}
                        <optgroup label="Local Templates">
                            {chunkTemplates.map(tmpl => (
                                <option key={tmpl.name} value={tmpl.name}>
                                    {tmpl.name}
                                </option>
                            ))}
                        </optgroup>
                    </select>
                    <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
                        {chunkTemplates.map(tmpl => (
                            <button
                                key={tmpl.name}
                                type="button"
                                onClick={() => {
                                    setTitle("");
                                    setContent(tmpl.content);
                                    setType(tmpl.type);
                                    setTags(tmpl.tags);
                                    setSelectedTemplateId(tmpl.name);
                                }}
                                className="hover:bg-muted rounded-md border p-3 text-left transition-colors"
                            >
                                <p className="text-sm font-medium">{tmpl.name}</p>
                                <p className="text-muted-foreground mt-0.5 text-xs">{tmpl.description}</p>
                            </button>
                        ))}
                    </div>
                </CardPanel>
            </Card>

            <Card>
                <CardPanel className="space-y-4 p-6">
                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Title</label>
                        <input
                            type="text"
                            value={title}
                            onChange={e => setTitle(e.target.value)}
                            placeholder="Enter a title..."
                            className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                        />
                        {errors.title && <p className="text-destructive mt-1 text-xs">{errors.title}</p>}
                        {duplicates.length > 0 && (
                            <div className="mt-2 rounded-md border border-yellow-500/30 bg-yellow-500/10 px-3 py-2">
                                <p className="text-xs font-medium text-yellow-600 dark:text-yellow-400">Similar chunks exist:</p>
                                <ul className="mt-1 space-y-0.5">
                                    {duplicates.map((d: { id: string; title: string }) => (
                                        <li key={d.id} className="text-xs">
                                            <Link
                                                to="/chunks/$chunkId"
                                                params={{ chunkId: d.id }}
                                                className="text-yellow-600 underline dark:text-yellow-400"
                                            >
                                                {d.title}
                                            </Link>
                                        </li>
                                    ))}
                                </ul>
                            </div>
                        )}
                    </div>

                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Type</label>
                        <select
                            value={type}
                            onChange={e => setType(e.target.value)}
                            className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                        >
                            <option value="note">Note</option>
                            <option value="document">Document</option>
                            <option value="reference">Reference</option>
                            <option value="schema">Schema</option>
                            <option value="checklist">Checklist</option>
                        </select>
                    </div>

                    <div>
                        <label className="mb-1.5 block text-sm font-medium">Tags</label>
                        <div className="mb-2 flex flex-wrap gap-2">
                            {tags.map(tag => (
                                <Badge
                                    key={tag}
                                    variant="secondary"
                                    size="sm"
                                    className="cursor-pointer"
                                    onClick={() => setTags(tags.filter(t => t !== tag))}
                                >
                                    {tag} ×
                                </Badge>
                            ))}
                        </div>
                        <input
                            type="text"
                            value={tagInput}
                            onChange={e => setTagInput(e.target.value)}
                            onKeyDown={e => {
                                if (e.key === "Enter") {
                                    e.preventDefault();
                                    addTag();
                                }
                            }}
                            placeholder="Add a tag and press Enter..."
                            className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                        />
                    </div>

                    <MarkdownEditor
                        value={content}
                        onChange={setContent}
                        placeholder="Write your content..."
                        rows={10}
                        error={errors.content}
                    />

                    <Separator />

                    {/* Applies-to repeatable field */}
                    <div>
                        <div className="mb-2 flex items-center justify-between">
                            <label className="text-sm font-medium">Applies To</label>
                            <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => setAppliesTo([...appliesTo, { pattern: "", note: "" }])}
                            >
                                <Plus className="mr-1 size-3" />
                                Add
                            </Button>
                        </div>
                        {appliesTo.map((row, i) => (
                            <div key={i} className="mb-2 flex gap-2">
                                <input
                                    type="text"
                                    value={row.pattern}
                                    onChange={e => {
                                        const updated = [...appliesTo];
                                        updated[i] = { ...updated[i], pattern: e.target.value };
                                        setAppliesTo(updated);
                                    }}
                                    placeholder="Pattern (e.g. src/**/*.ts)"
                                    className="bg-background focus:ring-ring flex-1 rounded-md border px-3 py-2 font-mono text-sm focus:ring-2 focus:outline-none"
                                />
                                <input
                                    type="text"
                                    value={row.note}
                                    onChange={e => {
                                        const updated = [...appliesTo];
                                        updated[i] = { ...updated[i], note: e.target.value };
                                        setAppliesTo(updated);
                                    }}
                                    placeholder="Note (optional)"
                                    className="bg-background focus:ring-ring w-40 rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                />
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => setAppliesTo(appliesTo.filter((_, j) => j !== i))}
                                >
                                    <Trash2 className="size-3.5" />
                                </Button>
                            </div>
                        ))}
                    </div>

                    {/* File references repeatable field */}
                    <div>
                        <div className="mb-2 flex items-center justify-between">
                            <label className="text-sm font-medium">File References</label>
                            <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => setFileRefs([...fileRefs, { path: "", anchor: "", relation: "documents" }])}
                            >
                                <Plus className="mr-1 size-3" />
                                Add
                            </Button>
                        </div>
                        {fileRefs.map((row, i) => (
                            <div key={i} className="mb-2 flex gap-2">
                                <input
                                    type="text"
                                    value={row.path}
                                    onChange={e => {
                                        const updated = [...fileRefs];
                                        updated[i] = { ...updated[i], path: e.target.value };
                                        setFileRefs(updated);
                                    }}
                                    placeholder="File path"
                                    className="bg-background focus:ring-ring flex-1 rounded-md border px-3 py-2 font-mono text-sm focus:ring-2 focus:outline-none"
                                />
                                <input
                                    type="text"
                                    value={row.anchor}
                                    onChange={e => {
                                        const updated = [...fileRefs];
                                        updated[i] = { ...updated[i], anchor: e.target.value };
                                        setFileRefs(updated);
                                    }}
                                    placeholder="Anchor (optional)"
                                    className="bg-background focus:ring-ring w-32 rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                />
                                <select
                                    value={row.relation}
                                    onChange={e => {
                                        const updated = [...fileRefs];
                                        updated[i] = {
                                            ...updated[i],
                                            relation: e.target.value as FileRefRow["relation"]
                                        };
                                        setFileRefs(updated);
                                    }}
                                    className="bg-background focus:ring-ring w-32 rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                >
                                    <option value="documents">documents</option>
                                    <option value="configures">configures</option>
                                    <option value="tests">tests</option>
                                    <option value="implements">implements</option>
                                </select>
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => setFileRefs(fileRefs.filter((_, j) => j !== i))}
                                >
                                    <Trash2 className="size-3.5" />
                                </Button>
                            </div>
                        ))}
                    </div>

                    {/* Decision context collapsible */}
                    <div>
                        <button
                            type="button"
                            onClick={() => setShowDecisionContext(!showDecisionContext)}
                            className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-sm font-medium transition-colors"
                        >
                            <ChevronDown
                                className={`size-4 transition-transform ${showDecisionContext ? "rotate-0" : "-rotate-90"}`}
                            />
                            Decision Context
                        </button>
                        {showDecisionContext && (
                            <div className="mt-3 space-y-3 rounded-md border p-4">
                                <div>
                                    <label className="mb-1.5 block text-sm font-medium">Rationale</label>
                                    <textarea
                                        value={rationale}
                                        onChange={e => setRationale(e.target.value)}
                                        placeholder="Why was this decision made?"
                                        rows={3}
                                        className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                    />
                                </div>
                                <div>
                                    <label className="mb-1.5 block text-sm font-medium">Alternatives Considered</label>
                                    <input
                                        type="text"
                                        value={alternativesInput}
                                        onChange={e => setAlternativesInput(e.target.value)}
                                        placeholder="Comma-separated alternatives"
                                        className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                    />
                                </div>
                                <div>
                                    <label className="mb-1.5 block text-sm font-medium">Consequences</label>
                                    <textarea
                                        value={consequences}
                                        onChange={e => setConsequences(e.target.value)}
                                        placeholder="What becomes easier or harder?"
                                        rows={3}
                                        className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                    />
                                </div>
                            </div>
                        )}
                    </div>

                    <Separator />

                    <div className="flex justify-end gap-2">
                        <Button variant="outline" render={<Link to="/dashboard" />}>
                            Cancel
                        </Button>
                        <Button
                            onClick={() => {
                                if (validate()) createMutation.mutate();
                            }}
                            disabled={createMutation.isPending}
                        >
                            {createMutation.isPending ? "Creating..." : "Create Chunk"}
                        </Button>
                    </div>
                </CardPanel>
            </Card>
        </div>
    );
}
