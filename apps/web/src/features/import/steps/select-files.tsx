"use client";

import { FolderUp } from "lucide-react";
import { useMemo, useRef } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useApiQuery } from "@/hooks/use-api-query";
import { api } from "@/utils/api";
import { buildTree, FileTree } from "../file-tree";
import type { FileEntry } from "../types";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface StepSelectFilesProps {
    files: FileEntry[];
    onFilesChange: (files: FileEntry[]) => void;
    selectedPaths: Set<string>;
    onSelectionChange: (paths: Set<string>) => void;
    codebaseId: string;
    onCodebaseChange: (id: string) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function StepSelectFiles({
    files,
    onFilesChange,
    selectedPaths,
    onSelectionChange,
    codebaseId,
    onCodebaseChange
}: StepSelectFilesProps) {
    const folderInputRef = useRef<HTMLInputElement>(null);

    const { data: codebases } = useApiQuery<{ id: string; name: string }[]>({
        queryKey: ["codebases"],
        queryFn: () => api.api.codebases.get(),
        fallback: []
    });

    const tree = useMemo(() => buildTree(files.map(f => f.path)), [files]);

    const selectedCount = selectedPaths.size;
    const deselectedCount = files.length - selectedCount;

    const handleFolderSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
        if (!e.target.files) return;
        const entries = await readFilesFromInput(e.target.files);
        onFilesChange(entries);
        onSelectionChange(new Set(entries.map(f => f.path)));
        e.target.value = "";
    };

    const handleSelectAll = () => {
        onSelectionChange(new Set(files.map(f => f.path)));
    };

    const handleDeselectAll = () => {
        onSelectionChange(new Set());
    };

    return (
        <div className="flex flex-col gap-4">
            {/* Top bar */}
            <div className="flex flex-wrap items-center gap-3">
                {/* Hidden folder input */}
                <input
                    ref={folderInputRef}
                    type="file"
                    // @ts-expect-error webkitdirectory is non-standard
                    webkitdirectory=""
                    className="hidden"
                    onChange={handleFolderSelect}
                />

                <Button
                    variant="outline"
                    size="sm"
                    onClick={() => folderInputRef.current?.click()}
                >
                    <FolderUp className="mr-1 size-3.5" />
                    Select Folder...
                </Button>

                {files.length > 0 && (
                    <Badge variant="secondary">
                        {files.length} file{files.length !== 1 ? "s" : ""}
                    </Badge>
                )}

                <div className="ml-auto min-w-48">
                    <select
                        className="border-input bg-background block w-full rounded-md border px-3 py-1.5 text-sm"
                        value={codebaseId}
                        onChange={e => onCodebaseChange(e.target.value)}
                    >
                        <option value="">Select a codebase... *</option>
                        {codebases?.map(c => (
                            <option key={c.id} value={c.id}>
                                {c.name}
                            </option>
                        ))}
                    </select>
                </div>
            </div>

            {/* Summary bar + select/deselect links */}
            {files.length > 0 && (
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                    <span>
                        {selectedCount} selected, {deselectedCount} deselected
                    </span>
                    <span>·</span>
                    <button
                        type="button"
                        className="text-primary hover:underline"
                        onClick={handleSelectAll}
                    >
                        Select all
                    </button>
                    <span>/</span>
                    <button
                        type="button"
                        className="text-primary hover:underline"
                        onClick={handleDeselectAll}
                    >
                        Deselect all
                    </button>
                </div>
            )}

            {/* File tree or empty state */}
            {files.length === 0 ? (
                <div
                    className="flex cursor-pointer flex-col items-center justify-center gap-3 rounded-lg border-2 border-dashed p-12"
                    onClick={() => folderInputRef.current?.click()}
                >
                    <FolderUp className="size-10 text-muted-foreground" />
                    <p className="text-sm text-muted-foreground">
                        Select a folder of markdown files to get started
                    </p>
                </div>
            ) : (
                <div className="overflow-auto rounded-md border max-h-[480px]">
                    <FileTree
                        nodes={tree}
                        selected={selectedPaths}
                        onSelectionChange={onSelectionChange}
                        mode="checkbox"
                    />
                </div>
            )}
        </div>
    );
}
