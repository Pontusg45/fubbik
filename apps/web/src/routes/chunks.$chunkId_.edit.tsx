import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, ChevronDown, Plus, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import {
    Dialog,
    DialogPopup,
    DialogHeader,
    DialogTitle,
    DialogDescription,
    DialogPanel,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { DraftIndicator } from "@/features/chunks/draft-indicator";
import { loadDraft, useAutosave } from "@/features/chunks/use-autosave";
import { useActiveFeatures } from "@/features/feature-flags/use-active-features";
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

function isValidGlob(pattern: string): boolean {
    if (!pattern.trim()) return true;
    const unmatched = (pattern.match(/\[/g) || []).length !== (pattern.match(/\]/g) || []).length;
    const emptyBraces = /\{\s*\}/.test(pattern);
    return !unmatched && !emptyBraces;
}

interface EditChunkDraft {
    title: string;
    content: string;
    type: string;
    tags: string[];
    appliesTo: ApplyToRow[];
    fileRefs: FileRefRow[];
    rationale: string;
    alternativesInput: string;
    consequences: string;
}

function SaveTargetDialog({
    open,
    onOpenChange,
    activeFeatures,
    onSave,
}: {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    activeFeatures: Array<{ id: string; name: string; color: string | null }>;
    onSave: (target: "base" | string) => void;
}) {
    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogPopup showCloseButton={false}>
                <DialogHeader>
                    <DialogTitle>Save to</DialogTitle>
                    <DialogDescription>
                        You have active features. Save changes to the base chunk or as a feature overlay.
                    </DialogDescription>
                </DialogHeader>
                <DialogPanel>
                    <div className="space-y-2">
                        <Button
                            variant="outline"
                            className="w-full justify-start"
                            onClick={() => onSave("base")}
                        >
                            Base chunk
                        </Button>
                        {activeFeatures.map(f => (
                            <Button
                                key={f.id}
                                variant="outline"
                                className="w-full justify-start gap-2"
                                onClick={() => onSave(f.id)}
                            >
                                <span
                                    className="size-2 rounded-full"
                                    style={{ backgroundColor: f.color ?? "#8b5cf6" }}
                                />
                                Feature: {f.name}
                            </Button>
                        ))}
                    </div>
                </DialogPanel>
            </DialogPopup>
        </Dialog>
    );
}

function EditChunk() {
    const { chunkId } = Route.useParams();
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const draftKey = `chunk-draft-edit-${chunkId}`;

    const [title, setTitle] = useState("");
    const [content, setContent] = useState("");
    const [type, setType] = useState("note");
    const [tagInput, setTagInput] = useState("");
    const [tags, setTags] = useState<string[]>([]);
    const [errors, setErrors] = useState<Record<string, string>>({});
    const [initialized, setInitialized] = useState(false);
    const [saveDialogOpen, setSaveDialogOpen] = useState(false);

    // New fields
    const [appliesTo, setAppliesTo] = useState<ApplyToRow[]>([]);
    const [fileRefs, setFileRefs] = useState<FileRefRow[]>([]);
    const [showDecisionContext, setShowDecisionContext] = useState(false);
    const [rationale, setRationale] = useState("");
    const [alternativesInput, setAlternativesInput] = useState("");
    const [consequences, setConsequences] = useState("");

    const draftRef = useRef(loadDraft<EditChunkDraft>(draftKey));

    const { activeFeatureIds } = useActiveFeatures();

    const { data: featuresData } = useQuery({
        queryKey: ["features"],
        queryFn: async () => unwrapEden(await api.api.features.get({ query: {} })),
        staleTime: 60_000,
    });

    const { data, isLoading, error } = useQuery({
        queryKey: ["chunk", chunkId],
        queryFn: async () => {
            return unwrapEden(await api.api.chunks({ id: chunkId }).get());
        }
    });

    useEffect(() => {
        if (data?.chunk && !initialized) {
            const draft = draftRef.current;

            // If there's a saved draft, restore it instead of server data
            if (draft && (draft.title || draft.content)) {
                setTitle(draft.title);
                setContent(draft.content);
                setType(draft.type);
                setTags(draft.tags);
                setAppliesTo(draft.appliesTo);
                setFileRefs(draft.fileRefs);
                setRationale(draft.rationale);
                setAlternativesInput(draft.alternativesInput);
                setConsequences(draft.consequences);
                if (draft.rationale || draft.alternativesInput || draft.consequences) {
                    setShowDecisionContext(true);
                }
                toast.info("Restored unsaved draft");
                setInitialized(true);
                return;
            }

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

    // Autosave form state
    const formState = useMemo<EditChunkDraft>(
        () => ({ title, content, type, tags, appliesTo, fileRefs, rationale, alternativesInput, consequences }),
        [title, content, type, tags, appliesTo, fileRefs, rationale, alternativesInput, consequences]
    );
    const { clearDraft, lastSaved } = useAutosave(draftKey, formState, initialized);

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
            clearDraft();
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

    const deltasMutation = useMutation({
        mutationFn: async (featureId: string) => {
            const originalChunk = data?.chunk as Record<string, unknown> | undefined;
            if (!originalChunk) throw new Error("Original chunk data not available");
            const delta: Record<string, unknown> = {};
            if (title !== originalChunk.title) delta.title = title;
            if (content !== originalChunk.content) delta.content = content;
            if (type !== originalChunk.type) delta.type = type;
            const originalRationale = (originalChunk.rationale as string | null | undefined) ?? "";
            if (rationale !== originalRationale) delta.rationale = rationale;
            const originalConsequences = (originalChunk.consequences as string | null | undefined) ?? "";
            if (consequences !== originalConsequences) delta.consequences = consequences;
            if (Object.keys(delta).length === 0) {
                throw new Error("No changes to save as feature overlay");
            }
            await unwrapEden(
                await (api.api.chunks({ id: chunkId }) as any).deltas({ featureId }).put({ delta })
            );
        },
        onSuccess: () => {
            clearDraft();
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            toast.success("Saved as feature overlay");
            navigate({ to: "/chunks/$chunkId", params: { chunkId } });
        },
        onError: (err: unknown) => {
            const msg = err instanceof Error ? err.message : "Failed to save feature overlay";
            toast.error(msg);
        }
    });

    function handleSaveTarget(target: "base" | string) {
        setSaveDialogOpen(false);
        if (target === "base") {
            updateMutation.mutate();
        } else {
            deltasMutation.mutate(target);
        }
    }

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
                        <label htmlFor="edit-title" className="mb-1.5 block text-sm font-medium">Title</label>
                        <Input
                            id="edit-title"
                            type="text"
                            value={title}
                            onChange={e => setTitle(e.target.value)}
                            placeholder="Enter a title..."
                        />
                        {errors.title && <p className="text-destructive mt-1 text-xs">{errors.title}</p>}
                    </div>

                    <div>
                        <label htmlFor="edit-type" className="mb-1.5 block text-sm font-medium">Type</label>
                        <select
                            id="edit-type"
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
                        <label htmlFor="edit-tags" className="mb-1.5 block text-sm font-medium">Tags</label>
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
                        <Input
                            id="edit-tags"
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
                            <div key={i} className="mb-2">
                                <div className="flex gap-2">
                                    <Input
                                        type="text"
                                        value={row.pattern}
                                        onChange={e => {
                                            setAppliesTo(appliesTo.map((a, j) => (j === i ? { ...a, pattern: e.target.value } : a)));
                                        }}
                                        placeholder="Pattern (e.g. src/**/*.ts)"
                                        className="flex-1 font-mono"
                                    />
                                    <Input
                                        type="text"
                                        value={row.note}
                                        onChange={e => {
                                            setAppliesTo(appliesTo.map((a, j) => (j === i ? { ...a, note: e.target.value } : a)));
                                        }}
                                        placeholder="Note (optional)"
                                        className="w-40"
                                    />
                                    <Button
                                        variant="ghost"
                                        size="sm"
                                        onClick={() => setAppliesTo(appliesTo.filter((_, j) => j !== i))}
                                    >
                                        <Trash2 className="size-3.5" />
                                    </Button>
                                </div>
                                {row.pattern && !isValidGlob(row.pattern) && (
                                    <p className="text-destructive mt-1 text-xs">Invalid glob pattern</p>
                                )}
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
                                <Input
                                    type="text"
                                    value={row.path}
                                    onChange={e => {
                                        setFileRefs(fileRefs.map((f, j) => (j === i ? { ...f, path: e.target.value } : f)));
                                    }}
                                    placeholder="File path"
                                    className="flex-1 font-mono"
                                />
                                <Input
                                    type="text"
                                    value={row.anchor}
                                    onChange={e => {
                                        setFileRefs(fileRefs.map((f, j) => (j === i ? { ...f, anchor: e.target.value } : f)));
                                    }}
                                    placeholder="Anchor (optional)"
                                    className="w-32"
                                />
                                <select
                                    value={row.relation}
                                    onChange={e => {
                                        setFileRefs(
                                            fileRefs.map((f, j) =>
                                                j === i ? { ...f, relation: e.target.value as FileRefRow["relation"] } : f
                                            )
                                        );
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
                            aria-expanded={showDecisionContext}
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
                                    <label htmlFor="edit-rationale" className="mb-1.5 block text-sm font-medium">Rationale</label>
                                    <textarea
                                        id="edit-rationale"
                                        value={rationale}
                                        onChange={e => setRationale(e.target.value)}
                                        placeholder="Why was this decision made?"
                                        rows={3}
                                        className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                    />
                                </div>
                                <div>
                                    <label htmlFor="edit-alternatives" className="mb-1.5 block text-sm font-medium">Alternatives Considered</label>
                                    <Input
                                        id="edit-alternatives"
                                        type="text"
                                        value={alternativesInput}
                                        onChange={e => setAlternativesInput(e.target.value)}
                                        placeholder="Comma-separated alternatives"
                                    />
                                </div>
                                <div>
                                    <label htmlFor="edit-consequences" className="mb-1.5 block text-sm font-medium">Consequences</label>
                                    <textarea
                                        id="edit-consequences"
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

                    <div className="flex items-center justify-end gap-2">
                        <DraftIndicator lastSaved={lastSaved} />
                        <Button variant="outline" render={<Link to="/chunks/$chunkId" params={{ chunkId }} />}>
                            Cancel
                        </Button>
                        <Button
                            onClick={() => {
                                if (!validate()) return;
                                if ((activeFeatureIds as string[]).length > 0) {
                                    setSaveDialogOpen(true);
                                } else {
                                    updateMutation.mutate();
                                }
                            }}
                            disabled={updateMutation.isPending || deltasMutation.isPending}
                        >
                            {updateMutation.isPending || deltasMutation.isPending ? "Saving..." : "Save Changes"}
                        </Button>
                    </div>
                </CardPanel>
            </Card>

            <SaveTargetDialog
                open={saveDialogOpen}
                onOpenChange={setSaveDialogOpen}
                activeFeatures={(featuresData as Array<{ id: string; name: string; color: string | null }> | undefined ?? []).filter(
                    f => (activeFeatureIds as string[]).includes(f.id)
                )}
                onSave={handleSaveTarget}
            />
        </div>
    );
}
