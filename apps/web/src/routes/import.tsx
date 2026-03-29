import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import {
    AlertTriangle,
    CheckCircle2,
    ChevronDown,
    ChevronRight,
    FileText,
    FolderUp,
    Upload,
    XCircle
} from "lucide-react";
import { useRef, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { PageContainer, PageHeader } from "@/components/ui/page";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/import")({
    component: ImportPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

// ─── Types ───

interface FileEntry {
    path: string;
    content: string;
}

interface PreviewEntry {
    path: string;
    content: string;
    title: string;
    tags: string[];
    type: string;
    selected: boolean;
}

interface ImportResult {
    created: number;
    skipped: number;
    errors: Array<{ path: string; error: string }>;
}

// ─── Helpers ───

function previewFile(path: string, content: string): Omit<PreviewEntry, "selected"> {
    let title = path.split("/").pop()?.replace(/\.md$/, "") ?? path;
    let type = "document";
    const tags: string[] = [];

    // Extract frontmatter
    const fmMatch = content.match(/^---\n([\s\S]*?)\n---/);
    if (fmMatch) {
        const fm = fmMatch[1];
        const titleMatch = fm.match(/^title:\s*(.+)$/m);
        if (titleMatch) title = titleMatch[1].replace(/^["']|["']$/g, "");
        const typeMatch = fm.match(/^type:\s*(.+)$/m);
        if (typeMatch) type = typeMatch[1].trim();
        const tagsMatch = fm.match(/^tags:\s*\[(.+)\]$/m);
        if (tagsMatch) {
            tags.push(...tagsMatch[1].split(",").map(t => t.trim().replace(/^["']|["']$/g, "")));
        }
    }

    // Fallback to H1 heading
    if (!fmMatch) {
        const h1Match = content.match(/^#\s+(.+)$/m);
        if (h1Match) title = h1Match[1];
    }

    // Derive tags from folder structure
    const parts = path.split("/");
    if (parts.length > 1) {
        // Use parent folders as tags (exclude the filename)
        for (let i = 0; i < parts.length - 1; i++) {
            const folder = parts[i];
            if (folder && !tags.includes(folder)) {
                tags.push(folder);
            }
        }
    }

    return { path, content, title, tags, type };
}

function readFilesFromInput(fileList: FileList): Promise<FileEntry[]> {
    const mdFiles = Array.from(fileList).filter(f => f.name.endsWith(".md"));
    return Promise.all(
        mdFiles.map(
            file =>
                new Promise<FileEntry>((resolve, reject) => {
                    const reader = new FileReader();
                    reader.onload = () =>
                        resolve({
                            path: file.webkitRelativePath || file.name,
                            content: reader.result as string
                        });
                    reader.onerror = () => reject(reader.error);
                    reader.readAsText(file);
                })
        )
    );
}

// ─── Page Component ───

function ImportPage() {
    const queryClient = useQueryClient();
    const folderInputRef = useRef<HTMLInputElement>(null);
    const [previews, setPreviews] = useState<PreviewEntry[]>([]);
    const [codebaseId, setCodebaseId] = useState<string>("");
    const [result, setResult] = useState<ImportResult | null>(null);
    const [showErrors, setShowErrors] = useState(false);

    const { data: codebases } = useQuery({
        queryKey: ["codebases"],
        queryFn: async () => unwrapEden(await api.api.codebases.get()),
        staleTime: 60_000
    });

    const importMutation = useMutation({
        mutationFn: async (payload: { files: FileEntry[]; codebaseId: string }) =>
            unwrapEden(
                await api.api.chunks["import-docs"].post({
                    files: payload.files,
                    codebaseId: payload.codebaseId
                })
            ),
        onSuccess: (data: ImportResult) => {
            setResult(data);
            const msg = `Created: ${data.created} | Skipped: ${data.skipped} | Errors: ${data.errors.length}`;
            if (data.errors.length > 0) {
                toast.warning(msg);
            } else {
                toast.success(msg);
            }
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
        },
        onError: () => toast.error("Failed to import docs")
    });

    const handleFolderSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
        if (!e.target.files) return;
        const entries = await readFilesFromInput(e.target.files);
        const previewed = entries.map(f => ({ ...previewFile(f.path, f.content), selected: true }));
        setPreviews(previewed);
        setResult(null);
        e.target.value = "";
    };

    const selectedCount = previews.filter(p => p.selected).length;

    const toggleAll = () => {
        const allSelected = selectedCount === previews.length;
        setPreviews(prev => prev.map(p => ({ ...p, selected: !allSelected })));
    };

    const toggleRow = (index: number) => {
        setPreviews(prev => prev.map((p, i) => (i === index ? { ...p, selected: !p.selected } : p)));
    };

    const handleImport = () => {
        if (!codebaseId) {
            toast.error("Please select a codebase");
            return;
        }
        const selected = previews.filter(p => p.selected);
        if (selected.length === 0) {
            toast.error("No files selected for import");
            return;
        }
        if (selected.length > 500) {
            toast.error("Too many files (max 500). Deselect some or import in batches.");
            return;
        }
        importMutation.mutate({
            files: selected.map(p => ({ path: p.path, content: p.content })),
            codebaseId
        });
    };

    return (
        <PageContainer maxWidth="5xl">
            <PageHeader
                icon={Upload}
                title="Import Docs"
                description="Import a folder of markdown files as knowledge chunks"
            />

            {/* Controls */}
            <div className="mb-6 flex flex-wrap items-end gap-4">
                <div>
                    <label className="text-sm font-medium">Folder</label>
                    <input
                        ref={folderInputRef}
                        type="file"
                        // @ts-expect-error webkitdirectory is non-standard
                        webkitdirectory=""
                        className="hidden"
                        onChange={handleFolderSelect}
                    />
                    <div className="mt-1">
                        <Button variant="outline" size="sm" onClick={() => folderInputRef.current?.click()}>
                            <FolderUp className="mr-1 size-3.5" />
                            Select Folder
                        </Button>
                    </div>
                </div>

                <div className="min-w-48">
                    <label className="text-sm font-medium">
                        Codebase <span className="text-destructive">*</span>
                    </label>
                    <select
                        className="border-input bg-background mt-1 block w-full rounded-md border px-3 py-2 text-sm"
                        value={codebaseId}
                        onChange={e => setCodebaseId(e.target.value)}
                    >
                        <option value="">Select a codebase...</option>
                        {codebases?.map((c: { id: string; name: string }) => (
                            <option key={c.id} value={c.id}>
                                {c.name}
                            </option>
                        ))}
                    </select>
                </div>

                {previews.length > 0 && (
                    <Button
                        size="sm"
                        onClick={handleImport}
                        disabled={importMutation.isPending || selectedCount === 0 || !codebaseId}
                    >
                        <Upload className="mr-1 size-3.5" />
                        {importMutation.isPending
                            ? "Importing..."
                            : `Import ${selectedCount} file${selectedCount !== 1 ? "s" : ""}`}
                    </Button>
                )}
            </div>

            {/* Results Summary */}
            {result && (
                <Card className="mb-6">
                    <CardHeader>
                        <CardTitle>Import Results</CardTitle>
                    </CardHeader>
                    <CardPanel>
                        <div className="flex flex-wrap gap-6">
                            <div className="flex items-center gap-2">
                                <CheckCircle2 className="text-success size-5" />
                                <span className="text-sm font-medium">Created: {result.created}</span>
                            </div>
                            <div className="flex items-center gap-2">
                                <AlertTriangle className="text-warning size-5" />
                                <span className="text-sm font-medium">Skipped: {result.skipped}</span>
                            </div>
                            {result.errors.length > 0 && (
                                <div className="flex items-center gap-2">
                                    <XCircle className="text-destructive size-5" />
                                    <span className="text-sm font-medium">Errors: {result.errors.length}</span>
                                </div>
                            )}
                        </div>

                        {result.errors.length > 0 && (
                            <div className="mt-4">
                                <button
                                    type="button"
                                    className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-sm"
                                    onClick={() => setShowErrors(!showErrors)}
                                >
                                    {showErrors ? (
                                        <ChevronDown className="size-4" />
                                    ) : (
                                        <ChevronRight className="size-4" />
                                    )}
                                    Show error details
                                </button>
                                {showErrors && (
                                    <div className="mt-2 space-y-1">
                                        {result.errors.map((err, i) => (
                                            <div key={i} className="text-destructive-foreground bg-destructive/8 rounded px-3 py-2 text-sm">
                                                <span className="font-mono text-xs">{err.path}</span>
                                                <span className="mx-2">—</span>
                                                {err.error}
                                            </div>
                                        ))}
                                    </div>
                                )}
                            </div>
                        )}

                        <div className="mt-4">
                            <Link to="/chunks">
                                <Button variant="outline" size="sm">
                                    View Chunks
                                </Button>
                            </Link>
                        </div>
                    </CardPanel>
                </Card>
            )}

            {/* Empty State */}
            {previews.length === 0 && !result && (
                <div
                    className="flex cursor-pointer flex-col items-center justify-center gap-3 rounded-lg border-2 border-dashed p-12"
                    onClick={() => folderInputRef.current?.click()}
                >
                    <FolderUp className="text-muted-foreground size-10" />
                    <p className="text-muted-foreground text-sm">
                        Select a folder of markdown files to preview and import
                    </p>
                </div>
            )}

            {/* Preview Table */}
            {previews.length > 0 && (
                <div className="overflow-x-auto rounded-lg border">
                    <table className="w-full text-sm">
                        <thead>
                            <tr className="bg-muted/50 border-b">
                                <th className="w-10 px-3 py-2">
                                    <Checkbox
                                        checked={selectedCount === previews.length && previews.length > 0}
                                        indeterminate={selectedCount > 0 && selectedCount < previews.length}
                                        onCheckedChange={toggleAll}
                                    />
                                </th>
                                <th className="px-3 py-2 text-left font-medium">Path</th>
                                <th className="px-3 py-2 text-left font-medium">Title</th>
                                <th className="px-3 py-2 text-left font-medium">Tags</th>
                                <th className="w-24 px-3 py-2 text-left font-medium">Type</th>
                            </tr>
                        </thead>
                        <tbody>
                            {previews.map((preview, index) => (
                                <tr
                                    key={preview.path}
                                    className="hover:bg-muted/30 border-b last:border-b-0"
                                >
                                    <td className="px-3 py-2">
                                        <Checkbox
                                            checked={preview.selected}
                                            onCheckedChange={() => toggleRow(index)}
                                        />
                                    </td>
                                    <td className="max-w-48 truncate px-3 py-2 font-mono text-xs">
                                        <span className="flex items-center gap-1.5">
                                            <FileText className="text-muted-foreground size-3.5 shrink-0" />
                                            {preview.path}
                                        </span>
                                    </td>
                                    <td className="px-3 py-2">{preview.title}</td>
                                    <td className="px-3 py-2">
                                        <div className="flex flex-wrap gap-1">
                                            {preview.tags.map(tag => (
                                                <Badge key={tag} variant="secondary" size="sm">
                                                    {tag}
                                                </Badge>
                                            ))}
                                        </div>
                                    </td>
                                    <td className="px-3 py-2">
                                        <Badge variant="outline" size="sm">
                                            {preview.type}
                                        </Badge>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            )}

            {previews.length > 0 && (
                <p className="text-muted-foreground mt-2 text-xs">
                    {selectedCount} of {previews.length} file{previews.length !== 1 ? "s" : ""} selected
                    for import. Tags and types shown are previews — the server does the final parsing.
                </p>
            )}
        </PageContainer>
    );
}
