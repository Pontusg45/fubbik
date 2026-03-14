import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, ChevronDown, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { MarkdownEditor } from "@/features/editor/markdown-editor";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/chunks/$chunkId_/edit")({
    component: EditChunk,
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

function EditChunk() {
    const { chunkId } = Route.useParams();
    const navigate = useNavigate();
    const queryClient = useQueryClient();

    const [title, setTitle] = useState("");
    const [content, setContent] = useState("");
    const [type, setType] = useState("note");
    const [tagInput, setTagInput] = useState("");
    const [tags, setTags] = useState<string[]>([]);
    const [errors, setErrors] = useState<Record<string, string>>({});
    const [initialized, setInitialized] = useState(false);

    // New fields
    const [appliesTo, setAppliesTo] = useState<ApplyToRow[]>([]);
    const [fileRefs, setFileRefs] = useState<FileRefRow[]>([]);
    const [showDecisionContext, setShowDecisionContext] = useState(false);
    const [rationale, setRationale] = useState("");
    const [alternativesInput, setAlternativesInput] = useState("");
    const [consequences, setConsequences] = useState("");

    const { data, isLoading, error } = useQuery({
        queryKey: ["chunk", chunkId],
        queryFn: async () => {
            return unwrapEden(await api.api.chunks({ id: chunkId }).get());
        }
    });

    useEffect(() => {
        if (data?.chunk && !initialized) {
            const chunk = data.chunk;
            setTitle(chunk.title);
            setContent(chunk.content);
            setType(chunk.type);
            setTags([]);

            // Initialize decision context fields
            const chunkAny = chunk as Record<string, unknown>;
            if (chunkAny.rationale) {
                setRationale(chunkAny.rationale as string);
                setShowDecisionContext(true);
            }
            if (Array.isArray(chunkAny.alternatives) && (chunkAny.alternatives as string[]).length > 0) {
                setAlternativesInput((chunkAny.alternatives as string[]).join(", "));
                setShowDecisionContext(true);
            }
            if (chunkAny.consequences) {
                setConsequences(chunkAny.consequences as string);
                setShowDecisionContext(true);
            }

            // Initialize applies-to
            const dataAny = data as Record<string, unknown>;
            if (Array.isArray(dataAny.appliesTo)) {
                const items = dataAny.appliesTo as Array<{ pattern: string; note?: string | null }>;
                setAppliesTo(items.map(a => ({ pattern: a.pattern, note: a.note ?? "" })));
            }

            // Initialize file references
            if (Array.isArray(dataAny.fileReferences)) {
                const items = dataAny.fileReferences as Array<{
                    path: string;
                    anchor?: string | null;
                    relation: string;
                }>;
                setFileRefs(
                    items.map(f => ({
                        path: f.path,
                        anchor: f.anchor ?? "",
                        relation: f.relation as FileRefRow["relation"]
                    }))
                );
            }

            setInitialized(true);
        }
    }, [data, initialized]);

    function validate() {
        const e: Record<string, string> = {};
        if (!title.trim()) e.title = "Title is required";
        else if (title.length > 200) e.title = "Title must be 200 characters or less";
        if (content.length > 50000) e.content = "Content must be 50,000 characters or less";
        setErrors(e);
        return Object.keys(e).length === 0;
    }

    const updateMutation = useMutation({
        mutationFn: async () => {
            const alternatives = alternativesInput
                .split(",")
                .map(s => s.trim())
                .filter(Boolean);
            await unwrapEden(
                await api.api.chunks({ id: chunkId }).patch({
                    title,
                    content,
                    type,
                    tags,
                    ...(rationale ? { rationale } : {}),
                    ...(alternatives.length > 0 ? { alternatives } : {}),
                    ...(consequences ? { consequences } : {})
                })
            );

            // Update applies-to
            const validAppliesTo = appliesTo.filter(a => a.pattern.trim());
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

            // Update file refs
            const validFileRefs = fileRefs.filter(f => f.path.trim());
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
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            toast.success("Chunk updated");
            navigate({ to: "/chunks/$chunkId", params: { chunkId } });
        },
        onError: () => {
            toast.error("Failed to update chunk");
        }
    });

    const addTag = () => {
        const tag = tagInput.trim().toLowerCase();
        if (tag && !tags.includes(tag)) {
            setTags([...tags, tag]);
        }
        setTagInput("");
    };

    if (isLoading) {
        return (
            <div className="container mx-auto max-w-3xl px-4 py-8">
                <Button variant="ghost" size="sm" render={<Link to="/chunks/$chunkId" params={{ chunkId }} />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="mt-8 text-center">
                    <p className="text-muted-foreground">Loading...</p>
                </div>
            </div>
        );
    }

    if (error || !data?.chunk) {
        return (
            <div className="container mx-auto max-w-3xl px-4 py-8">
                <Button variant="ghost" size="sm" render={<Link to="/chunks/$chunkId" params={{ chunkId }} />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="mt-8 text-center">
                    <p className="text-muted-foreground">Chunk not found.</p>
                </div>
            </div>
        );
    }

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <Button variant="ghost" size="sm" render={<Link to="/chunks/$chunkId" params={{ chunkId }} />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
            </div>

            <h1 className="mb-6 text-2xl font-bold tracking-tight">Edit Chunk</h1>

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
                        <Button variant="outline" render={<Link to="/chunks/$chunkId" params={{ chunkId }} />}>
                            Cancel
                        </Button>
                        <Button
                            onClick={() => {
                                if (validate()) updateMutation.mutate();
                            }}
                            disabled={updateMutation.isPending}
                        >
                            {updateMutation.isPending ? "Saving..." : "Save Changes"}
                        </Button>
                    </div>
                </CardPanel>
            </Card>
        </div>
    );
}
