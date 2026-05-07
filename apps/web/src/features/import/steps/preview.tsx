"use client";

import { Loader2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { useApiQuery } from "@/hooks/use-api-query";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";
import { FileDetailPanel } from "../file-detail-panel";
import { buildTree, FileTree } from "../file-tree";
import type { FileConfig, FileEntry, PreviewFileResult } from "../types";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Derives folder-based tags from a file path by extracting parent directory
 * names, excluding common index/readme names.
 */
function deriveFolderTags(path: string): string[] {
    const parts = path.split("/");
    // Exclude the file name itself (last segment) and filter index-like names
    const excluded = new Set(["index", "readme", "_index"]);
    return parts
        .slice(0, -1)
        .map(p => p.toLowerCase().replace(/[^a-z0-9-_]/g, "-"))
        .filter(p => p.length > 0 && !excluded.has(p));
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface StepPreviewProps {
    files: FileEntry[];
    selectedPaths: Set<string>;
    codebaseId: string;
    preview: PreviewFileResult[];
    onPreviewLoaded: (results: PreviewFileResult[], hashes: Record<string, string>) => void;
    overrides: Map<string, FileConfig>;
    onOverridesChange: (overrides: Map<string, FileConfig>) => void;
    initialActivePath?: string;
}

// ---------------------------------------------------------------------------
// StepPreview component
// ---------------------------------------------------------------------------

export function StepPreview({
    files,
    selectedPaths,
    codebaseId,
    preview,
    onPreviewLoaded,
    overrides,
    onOverridesChange,
    initialActivePath
}: StepPreviewProps) {
    const [loading, setLoading] = useState(false);
    const [filter, setFilter] = useState("");
    const [activePath, setActivePath] = useState<string>(initialActivePath ?? "");

    const { data: rawTemplates } = useApiQuery<any[]>({
        queryKey: ["templates"],
        queryFn: () => api.api.templates.get(),
        fallback: []
    });

    const templates = useMemo(
        () => (rawTemplates ?? []).map((t: any) => ({ id: t.id, name: t.name })),
        [rawTemplates]
    );

    // ----- Load preview on mount if not already loaded -----
    useEffect(() => {
        if (preview.length > 0) return;

        const selectedFiles = files.filter(f => selectedPaths.has(f.path));
        if (selectedFiles.length === 0) return;

        const run = async () => {
            setLoading(true);
            try {
                const result = unwrapEden(
                    await api.api.chunks["import-docs"].preview.post({
                        files: selectedFiles.map(f => ({ path: f.path, content: f.content })),
                        codebaseId
                    })
                ) as { files: PreviewFileResult[]; existingHashes: Record<string, string> };

                onPreviewLoaded(result.files, result.existingHashes);

                // Initialize overrides from preview data
                const next = new Map<string, FileConfig>();
                for (const pf of result.files) {
                    const folderTags = deriveFolderTags(pf.path);
                    next.set(pf.path, {
                        title: pf.parsed.title,
                        type: pf.parsed.type,
                        tags: [...new Set([...pf.parsed.tags, ...folderTags])],
                        folderTags,
                        templateId: pf.suggestedTemplate?.id ?? null
                    });
                }
                onOverridesChange(next);

                // Auto-select first file (only if no initialActivePath was given)
                if (!initialActivePath && result.files.length > 0 && result.files[0]) {
                    setActivePath(result.files[0].path);
                }
            } catch {
                toast.error("Failed to load preview");
                // Fallback: initialize overrides from client-side file parsing
                const fallbackOverrides = new Map<string, FileConfig>();
                for (const file of selectedFiles) {
                    const folderTags = deriveFolderTags(file.path);
                    const title = extractTitle(file.path, file.content);
                    const type = extractType(file.content);
                    const tags = [...new Set([...extractFrontmatterTags(file.content), ...folderTags])];
                    fallbackOverrides.set(file.path, { title, type, tags, templateId: null, folderTags });
                }
                onOverridesChange(fallbackOverrides);
            } finally {
                setLoading(false);
            }
        };

        void run();
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    // Auto-select first file once preview loads (if activated externally and no path set)
    useEffect(() => {
        if (preview.length > 0 && activePath === "" && !initialActivePath && preview[0]) {
            setActivePath(preview[0].path);
        }
    }, [preview, activePath, initialActivePath]);

    // ----- Tree -----
    const selectedFiles = useMemo(
        () => files.filter(f => selectedPaths.has(f.path)),
        [files, selectedPaths]
    );

    const tree = useMemo(
        () => buildTree(selectedFiles.map(f => f.path)),
        [selectedFiles]
    );

    // Paths that have a suggested template match
    const templatePaths = useMemo(() => {
        const set = new Set<string>();
        for (const pf of preview) {
            if (pf.suggestedTemplate) set.add(pf.path);
        }
        return set;
    }, [preview]);

    // ----- Active file data -----
    const activePreview = activePath
        ? preview.find(p => p.path === activePath) ?? null
        : null;

    const activeConfig = activePath ? overrides.get(activePath) ?? null : null;

    const handleConfigChange = (path: string, config: FileConfig) => {
        const next = new Map(overrides);
        next.set(path, config);
        onOverridesChange(next);
    };

    // Sibling count for "apply to folder" hint
    const siblingCount = useMemo(() => {
        if (!activePath) return 0;
        const dir = activePath.includes("/")
            ? activePath.substring(0, activePath.lastIndexOf("/"))
            : "";
        return selectedFiles.filter(f => {
            const fDir = f.path.includes("/")
                ? f.path.substring(0, f.path.lastIndexOf("/"))
                : "";
            return fDir === dir;
        }).length;
    }, [activePath, selectedFiles]);

    const handleApplyToFolder = (templateId: string) => {
        if (!activePath) return;
        const dir = activePath.includes("/")
            ? activePath.substring(0, activePath.lastIndexOf("/"))
            : "";

        const next = new Map(overrides);
        for (const f of selectedFiles) {
            const fDir = f.path.includes("/")
                ? f.path.substring(0, f.path.lastIndexOf("/"))
                : "";
            if (fDir === dir) {
                const existing = next.get(f.path);
                if (existing) {
                    next.set(f.path, { ...existing, templateId });
                }
            }
        }
        onOverridesChange(next);
    };

    // ----- Loading state -----
    if (loading) {
        return (
            <div className="flex items-center justify-center py-16 gap-2 text-muted-foreground">
                <Loader2 className="size-5 animate-spin" />
                <span className="text-sm">Analyzing files…</span>
            </div>
        );
    }

    // ----- Layout -----
    return (
        <div
            style={{ display: "flex", height: "500px" }}
            className="rounded-md border overflow-hidden"
        >
            {/* Left panel — file tree */}
            <div className="w-80 shrink-0 border-r flex flex-col">
                <div className="p-2 border-b">
                    <input
                        type="text"
                        className="border-input bg-background block w-full rounded-md border px-2.5 py-1 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
                        placeholder="Filter files…"
                        value={filter}
                        onChange={e => setFilter(e.target.value)}
                    />
                </div>
                <div className="flex-1 overflow-y-auto p-1">
                    <FileTree
                        nodes={tree}
                        selected={selectedPaths}
                        onSelectionChange={() => {
                            /* read-only in navigate mode */
                        }}
                        mode="navigate"
                        activePath={activePath ?? undefined}
                        onFileClick={setActivePath}
                        templatePaths={templatePaths}
                        filter={filter}
                    />
                </div>
            </div>

            {/* Right panel — file detail editor */}
            <div className="flex-1 overflow-hidden">
                {activePreview && activeConfig ? (
                    <FileDetailPanel
                        filePath={activePath}
                        preview={activePreview}
                        config={activeConfig}
                        onConfigChange={handleConfigChange}
                        templates={templates}
                        siblingCount={siblingCount}
                        onApplyToFolder={handleApplyToFolder}
                    />
                ) : (
                    <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
                        Select a file to configure
                    </div>
                )}
            </div>
        </div>
    );
}

// ---------------------------------------------------------------------------
// Fallback parsing helpers (used when preview API fails)
// ---------------------------------------------------------------------------

function extractTitle(path: string, content: string): string {
    const fmMatch = content.match(/^---\n[\s\S]*?title:\s*(.+)\n[\s\S]*?---/);
    if (fmMatch?.[1]) return fmMatch[1].replace(/^["']|["']$/g, "");
    const h1Match = content.match(/^#\s+(.+)$/m);
    if (h1Match?.[1]) return h1Match[1];
    return path.split("/").pop()?.replace(/\.md$/, "").replace(/-/g, " ") ?? path;
}

function extractType(content: string): string {
    const fmMatch = content.match(/^---\n[\s\S]*?type:\s*(.+)\n[\s\S]*?---/);
    return fmMatch?.[1]?.trim() ?? "document";
}

function extractFrontmatterTags(content: string): string[] {
    const fmMatch = content.match(/^---\n([\s\S]*?)---/);
    if (!fmMatch) return [];
    const tagsMatch = fmMatch[1]?.match(/tags:\s*\n((?:\s+-\s+.+\n)*)/);
    if (tagsMatch?.[1]) {
        return tagsMatch[1].split("\n").map(l => l.replace(/^\s+-\s+/, "").trim()).filter(Boolean);
    }
    const inlineMatch = fmMatch[1]?.match(/tags:\s*\[(.+)\]/);
    if (inlineMatch?.[1]) {
        return inlineMatch[1].split(",").map(t => t.trim().replace(/^["']|["']$/g, ""));
    }
    return [];
}
