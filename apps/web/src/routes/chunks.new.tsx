import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, redirect, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, FileText, Loader2, Sparkles } from "lucide-react";
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
            return unwrapEden(
                await api.api.chunks.post({
                    title,
                    content,
                    type,
                    tags
                })
            );
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
