import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Copy, Eye, FileText, LayoutTemplate, Pencil, Plus, Trash2, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Empty, EmptyAction, EmptyDescription, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/templates")({
    component: TemplatesPage,
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

interface Template {
    id: string;
    name: string;
    description: string | null;
    type: string;
    content: string;
    isBuiltIn: boolean;
    userId: string | null;
}

function TemplatesPage() {
    const queryClient = useQueryClient();
    const [showForm, setShowForm] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);
    const [previewTemplate, setPreviewTemplate] = useState<Template | null>(null);
    const [deleteTarget, setDeleteTarget] = useState<{ id: string; name: string } | null>(null);
    const [name, setName] = useState("");
    const [description, setDescription] = useState("");
    const [formType, setFormType] = useState("note");
    const [formContent, setFormContent] = useState("");

    const templatesQuery = useQuery({
        queryKey: ["templates"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.templates.get()) as Template[];
            } catch {
                return [];
            }
        }
    });

    const templates = Array.isArray(templatesQuery.data) ? templatesQuery.data : [];

    const createMutation = useMutation({
        mutationFn: async (body: { name: string; description?: string; type: string; content: string }) => {
            return unwrapEden(await api.api.templates.post(body));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["templates"] });
            resetForm();
            toast.success("Template created");
        },
        onError: () => {
            toast.error("Failed to create template");
        }
    });

    const updateMutation = useMutation({
        mutationFn: async ({ id, body }: { id: string; body: { name?: string; description?: string; type?: string; content?: string } }) => {
            return unwrapEden(await api.api.templates({ id }).patch(body));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["templates"] });
            resetForm();
            toast.success("Template updated");
        },
        onError: () => {
            toast.error("Failed to update template");
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => {
            return unwrapEden(await api.api.templates({ id }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["templates"] });
            toast.success("Template deleted");
        },
        onError: () => {
            toast.error("Failed to delete template");
        }
    });

    function resetForm() {
        setShowForm(false);
        setEditingId(null);
        setName("");
        setDescription("");
        setFormType("note");
        setFormContent("");
    }

    function handleDuplicate(template: Template) {
        setName(template.name + " (copy)");
        setDescription(template.description ?? "");
        setFormType(template.type);
        setFormContent(template.content);
        setEditingId(null);
        setShowForm(true);
    }

    function handleEdit(template: Template) {
        setName(template.name);
        setDescription(template.description ?? "");
        setFormType(template.type);
        setFormContent(template.content);
        setEditingId(template.id);
        setShowForm(true);
    }

    function handleDelete(id: string, templateName: string) {
        setDeleteTarget({ id, name: templateName });
    }

    function handleSubmit(e: React.FormEvent) {
        e.preventDefault();
        if (!name.trim() || !formContent.trim()) return;

        if (editingId) {
            updateMutation.mutate({
                id: editingId,
                body: {
                    name: name.trim(),
                    ...(description.trim() ? { description: description.trim() } : {}),
                    type: formType,
                    content: formContent
                }
            });
        } else {
            createMutation.mutate({
                name: name.trim(),
                ...(description.trim() ? { description: description.trim() } : {}),
                type: formType,
                content: formContent
            });
        }
    }

    function getContentPreview(content: string): string {
        const lines = content.split("\n").slice(0, 2);
        const preview = lines.join(" ").slice(0, 120);
        return content.length > 120 || content.split("\n").length > 2 ? preview + "..." : preview;
    }

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6 flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <FileText className="size-5" />
                    <h1 className="text-2xl font-bold tracking-tight">Templates</h1>
                    <Badge variant="secondary" className="ml-2">
                        {templates.length}
                    </Badge>
                </div>
                {!showForm && (
                    <Button size="sm" onClick={() => setShowForm(true)}>
                        <Plus className="mr-1 size-4" />
                        New Template
                    </Button>
                )}
            </div>

            {showForm && (
                <Card className="mb-6">
                    <CardPanel className="p-6">
                        <form onSubmit={handleSubmit} className="space-y-3">
                            <h2 className="text-sm font-medium">{editingId ? "Edit Template" : "Create Template"}</h2>
                            <div className="flex flex-col gap-2 sm:flex-row">
                                <input
                                    type="text"
                                    placeholder="Name"
                                    value={name}
                                    onChange={e => setName(e.target.value)}
                                    required
                                    className="bg-background focus:ring-ring flex-1 rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                />
                                <select
                                    value={formType}
                                    onChange={e => setFormType(e.target.value)}
                                    className="bg-background focus:ring-ring w-32 rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                                >
                                    <option value="note">Note</option>
                                    <option value="document">Document</option>
                                    <option value="reference">Reference</option>
                                    <option value="schema">Schema</option>
                                    <option value="checklist">Checklist</option>
                                </select>
                            </div>
                            <input
                                type="text"
                                placeholder="Description (optional)"
                                value={description}
                                onChange={e => setDescription(e.target.value)}
                                className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                            />
                            <textarea
                                placeholder="Template content..."
                                value={formContent}
                                onChange={e => setFormContent(e.target.value)}
                                required
                                rows={8}
                                className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 font-mono text-sm focus:ring-2 focus:outline-none"
                            />
                            <div className="flex justify-end gap-2">
                                <Button type="button" variant="outline" size="sm" onClick={resetForm}>
                                    Cancel
                                </Button>
                                <Button
                                    type="submit"
                                    size="sm"
                                    disabled={createMutation.isPending || updateMutation.isPending || !name.trim() || !formContent.trim()}
                                >
                                    {editingId
                                        ? updateMutation.isPending
                                            ? "Saving..."
                                            : "Save"
                                        : createMutation.isPending
                                          ? "Creating..."
                                          : "Create"}
                                </Button>
                            </div>
                        </form>
                    </CardPanel>
                </Card>
            )}

            <Card>
                <CardPanel className="p-6">
                    {templatesQuery.isLoading ? (
                        <SkeletonList count={4} />
                    ) : templates.length === 0 ? (
                        <Empty>
                            <EmptyMedia variant="icon"><LayoutTemplate className="h-10 w-10" /></EmptyMedia>
                            <EmptyTitle>No custom templates</EmptyTitle>
                            <EmptyDescription>Templates pre-fill chunk forms for common patterns.</EmptyDescription>
                            <EmptyAction>
                                <Button onClick={() => setShowForm(true)}>Create Template</Button>
                            </EmptyAction>
                        </Empty>
                    ) : (
                        <div className="divide-y">
                            {templates.map(t => (
                                <div key={t.id} className="flex items-start justify-between gap-4 py-3 first:pt-0 last:pb-0">
                                    <div className="min-w-0 flex-1">
                                        <div className="flex items-center gap-2">
                                            <button
                                                onClick={() => setPreviewTemplate(t)}
                                                className="hover:text-primary text-left font-medium transition-colors hover:underline"
                                            >
                                                {t.name}
                                            </button>
                                            <Badge variant="secondary" size="sm" className="text-[10px]">
                                                {t.type}
                                            </Badge>
                                            {t.isBuiltIn && (
                                                <Badge variant="outline" size="sm" className="text-[10px]">
                                                    Built-in
                                                </Badge>
                                            )}
                                        </div>
                                        {t.description && (
                                            <p className="text-muted-foreground mt-0.5 text-sm">{t.description}</p>
                                        )}
                                        <p className="text-muted-foreground mt-1 truncate font-mono text-xs">
                                            {getContentPreview(t.content)}
                                        </p>
                                    </div>
                                    <div className="flex gap-1">
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => setPreviewTemplate(t)}
                                            title="Preview"
                                        >
                                            <Eye className="size-3.5" />
                                        </Button>
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            onClick={() => handleDuplicate(t)}
                                            title="Duplicate"
                                        >
                                            <Copy className="size-3.5" />
                                        </Button>
                                        {!t.isBuiltIn && (
                                            <>
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    onClick={() => handleEdit(t)}
                                                    title="Edit"
                                                >
                                                    <Pencil className="size-3.5" />
                                                </Button>
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    onClick={() => handleDelete(t.id, t.name)}
                                                    disabled={deleteMutation.isPending}
                                                    title="Delete"
                                                >
                                                    <Trash2 className="size-3.5" />
                                                </Button>
                                            </>
                                        )}
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </CardPanel>
            </Card>

            <ConfirmDialog
                open={deleteTarget !== null}
                onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}
                title="Delete template"
                description={deleteTarget ? `Delete template "${deleteTarget.name}"?` : ""}
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

            {/* Preview dialog */}
            {previewTemplate && (
                <div className="fixed inset-0 z-50 flex items-center justify-center">
                    <div className="bg-black/50 absolute inset-0 backdrop-blur-sm" onClick={() => setPreviewTemplate(null)} />
                    <div className="bg-card relative mx-4 w-full max-w-2xl max-h-[80vh] overflow-hidden rounded-xl border shadow-2xl">
                        <div className="flex items-center justify-between border-b px-6 py-4">
                            <div className="flex items-center gap-3">
                                <FileText className="text-muted-foreground size-5" />
                                <div>
                                    <h2 className="font-semibold">{previewTemplate.name}</h2>
                                    <div className="flex items-center gap-2 mt-0.5">
                                        <Badge variant="secondary" size="sm" className="text-[10px]">
                                            {previewTemplate.type}
                                        </Badge>
                                        {previewTemplate.isBuiltIn && (
                                            <Badge variant="outline" size="sm" className="text-[10px]">
                                                Built-in
                                            </Badge>
                                        )}
                                    </div>
                                </div>
                            </div>
                            <Button variant="ghost" size="sm" onClick={() => setPreviewTemplate(null)}>
                                <X className="size-4" />
                            </Button>
                        </div>
                        {previewTemplate.description && (
                            <div className="text-muted-foreground border-b px-6 py-3 text-sm">
                                {previewTemplate.description}
                            </div>
                        )}
                        <div className="overflow-y-auto px-6 py-4" style={{ maxHeight: "calc(80vh - 140px)" }}>
                            <pre className="whitespace-pre-wrap font-mono text-sm leading-relaxed">{previewTemplate.content}</pre>
                        </div>
                        <div className="flex justify-end gap-2 border-t px-6 py-3">
                            <Button variant="outline" size="sm" onClick={() => { handleDuplicate(previewTemplate); setPreviewTemplate(null); }}>
                                <Copy className="size-3.5" />
                                Duplicate
                            </Button>
                            <Button size="sm" onClick={() => {
                                navigator.clipboard.writeText(previewTemplate.content);
                                toast.success("Content copied");
                            }}>
                                <Copy className="size-3.5" />
                                Copy Content
                            </Button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
