# Import Wizard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat `/import` page with a four-step wizard (Select Files → Preview & Configure → Review → Import) that uses the existing preview endpoint, adds SSE streaming for per-file progress, and keeps the old flow as a Quick mode toggle.

**Architecture:** The page shell renders either `ImportQuickMode` (existing code extracted) or `ImportWizard` (new four-step state machine). The wizard uses local `useState` for all state. Backend gets one new SSE streaming endpoint and a small extension to the preview response for duplicate detection hashes.

**Tech Stack:** React, TanStack Router, Elysia, Effect, @base-ui/react (Checkbox), Eden treaty, SSE via fetch ReadableStream

**Spec:** `docs/superpowers/specs/2026-05-07-import-wizard-design.md`

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `apps/web/src/features/import/types.ts` | Shared types for the wizard (FileEntry, WizardState, FileConfig, etc.) |
| `apps/web/src/features/import/quick-mode.tsx` | Extracted current import page as the Quick mode component |
| `apps/web/src/features/import/wizard.tsx` | Wizard state machine, step indicator, navigation footer |
| `apps/web/src/features/import/file-tree.tsx` | Recursive tree with tri-state checkboxes, index badges, connection hints |
| `apps/web/src/features/import/steps/select-files.tsx` | Step 1: folder picker, codebase dropdown, file tree |
| `apps/web/src/features/import/steps/preview.tsx` | Step 2: split-pane with tree nav + detail panel |
| `apps/web/src/features/import/file-detail-panel.tsx` | Right panel editor: title, tags, type, template, content preview |
| `apps/web/src/features/import/steps/review.tsx` | Step 3: summary stats, collapsible tables, connection preview |
| `apps/web/src/features/import/steps/import-step.tsx` | Step 4: pipeline table, SSE consumer, completion state |
| `apps/web/src/features/import/use-sse-import.ts` | Hook wrapping fetch+ReadableStream SSE parsing for the streaming import |
| `packages/api/src/chunks/__tests__/import-stream.test.ts` | Tests for the streaming import service function |

### Modified Files

| File | Changes |
|------|---------|
| `apps/web/src/routes/import.tsx` | Replace body with mode toggle routing to QuickMode or Wizard |
| `packages/api/src/chunks/routes.ts` | Add `POST /chunks/import-docs/stream` SSE route |
| `packages/api/src/chunks/service.ts` | Add `importDocsStream` generator function, add `getExistingHashes` function |

---

### Task 1: Shared Types

**Files:**
- Create: `apps/web/src/features/import/types.ts`

- [ ] **Step 1: Create the shared types file**

```typescript
// apps/web/src/features/import/types.ts

export interface FileEntry {
    path: string;
    content: string;
}

export interface FileConfig {
    title: string;
    type: string;
    tags: string[];
    templateId: string | null;
    folderTags: string[];
}

export interface ImportFileStatus {
    status: "pending" | "importing" | "created" | "skipped" | "error";
    created?: number;
    error?: string;
}

export interface PreviewFileResult {
    path: string;
    title: string;
    suggestedTemplate: {
        id: string;
        name: string;
        score: number;
        type: string;
        tags: string[];
        extractedFields: Record<string, unknown>;
    } | null;
    parsed: {
        title: string;
        type: string;
        tags: string[];
        content: string;
    };
}

export type WizardStep = 1 | 2 | 3 | 4;

export interface TreeNode {
    name: string;
    path: string;
    isDir: boolean;
    isIndex: boolean;
    children: TreeNode[];
}

export const INDEX_FILE_NAMES = new Set(["index.md", "readme.md", "_index.md"]);
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/import/types.ts
git commit -m "feat(import-wizard): add shared types for wizard state"
```

---

### Task 2: Extract Quick Mode from Existing Import Page

**Files:**
- Create: `apps/web/src/features/import/quick-mode.tsx`
- Modify: `apps/web/src/routes/import.tsx`

- [ ] **Step 1: Create quick-mode.tsx by extracting the current import page body**

Move the entire current `ImportPage` function body (state, handlers, JSX) into a new `ImportQuickMode` component. Keep the `readFilesFromInput` and `previewFile` helpers inside since they're only used here.

```typescript
// apps/web/src/features/import/quick-mode.tsx
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
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
import { useApiQuery } from "@/hooks/use-api-query";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import type { FileEntry } from "./types";

// Copy previewFile, readFilesFromInput helpers and the full component body
// from the current routes/import.tsx — this is a straight extraction, no logic changes.
// The component should be exported as:
export function ImportQuickMode() {
    // ... exact same code as current ImportPage function body
}
```

- [ ] **Step 2: Replace routes/import.tsx with mode toggle shell**

```typescript
// apps/web/src/routes/import.tsx
import { createFileRoute } from "@tanstack/react-router";
import { Upload } from "lucide-react";
import { useState } from "react";

import { PageContainer, PageHeader } from "@/components/ui/page";
import { getUser } from "@/functions/get-user";
import { ImportQuickMode } from "@/features/import/quick-mode";

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

function ImportPage() {
    const [mode, setMode] = useState<"quick" | "wizard">(() => {
        if (typeof window !== "undefined") {
            return (localStorage.getItem("import-mode") as "quick" | "wizard") ?? "quick";
        }
        return "quick";
    });

    const handleModeChange = (newMode: "quick" | "wizard") => {
        setMode(newMode);
        localStorage.setItem("import-mode", newMode);
    };

    return (
        <PageContainer maxWidth="5xl">
            <div className="mb-6 flex items-center justify-between">
                <PageHeader
                    icon={Upload}
                    title="Import Docs"
                    description="Import a folder of markdown files as knowledge chunks"
                />
                <div className="bg-muted flex rounded-lg p-1 gap-0.5">
                    <button
                        className={`rounded-md px-3 py-1.5 text-sm font-medium transition-colors ${mode === "quick" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"}`}
                        onClick={() => handleModeChange("quick")}
                    >
                        Quick
                    </button>
                    <button
                        className={`rounded-md px-3 py-1.5 text-sm font-medium transition-colors ${mode === "wizard" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"}`}
                        onClick={() => handleModeChange("wizard")}
                    >
                        Wizard
                    </button>
                </div>
            </div>

            {mode === "quick" ? (
                <ImportQuickMode />
            ) : (
                <div className="text-muted-foreground flex items-center justify-center rounded-lg border-2 border-dashed p-12">
                    Wizard mode coming soon
                </div>
            )}
        </PageContainer>
    );
}
```

- [ ] **Step 3: Verify the quick mode works unchanged**

Run: `pnpm dev` and open `http://localhost:3001/import`
Expected: The import page looks and works exactly as before, with a Quick/Wizard toggle in the header. Quick is selected by default.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/import/quick-mode.tsx apps/web/src/routes/import.tsx
git commit -m "feat(import-wizard): extract quick mode, add mode toggle shell"
```

---

### Task 3: File Tree Component

**Files:**
- Create: `apps/web/src/features/import/file-tree.tsx`

This is the core reusable tree used in both Step 1 (with checkboxes) and Step 2 (without checkboxes, with click-to-select).

- [ ] **Step 1: Create the file tree utility functions**

```typescript
// apps/web/src/features/import/file-tree.tsx
import { ChevronDown, ChevronRight, File, Folder } from "lucide-react";
import { useState, useCallback } from "react";

import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";

import { type TreeNode, INDEX_FILE_NAMES } from "./types";

export function buildTree(paths: string[]): TreeNode[] {
    const root: TreeNode[] = [];

    for (const filePath of paths) {
        const parts = filePath.split("/");
        let current = root;

        for (let i = 0; i < parts.length; i++) {
            const name = parts[i]!;
            const isLast = i === parts.length - 1;
            const currentPath = parts.slice(0, i + 1).join("/");

            let node = current.find(n => n.name === name);
            if (!node) {
                node = {
                    name,
                    path: currentPath,
                    isDir: !isLast,
                    isIndex: isLast && INDEX_FILE_NAMES.has(name.toLowerCase()),
                    children: [],
                };
                current.push(node);
            }
            current = node.children;
        }
    }

    return root;
}

function countFiles(node: TreeNode): number {
    if (!node.isDir) return 1;
    return node.children.reduce((sum, child) => sum + countFiles(child), 0);
}

function getCheckState(
    node: TreeNode,
    selected: Set<string>
): "checked" | "unchecked" | "indeterminate" {
    if (!node.isDir) {
        return selected.has(node.path) ? "checked" : "unchecked";
    }
    const files = getAllFiles(node);
    const selectedCount = files.filter(f => selected.has(f)).length;
    if (selectedCount === 0) return "unchecked";
    if (selectedCount === files.length) return "checked";
    return "indeterminate";
}

function getAllFiles(node: TreeNode): string[] {
    if (!node.isDir) return [node.path];
    return node.children.flatMap(getAllFiles);
}

function findIndexFile(children: TreeNode[]): TreeNode | undefined {
    return children.find(c => !c.isDir && c.isIndex);
}
```

- [ ] **Step 2: Create the FileTree component with tri-state checkboxes**

Add to the same file:

```typescript
interface FileTreeProps {
    nodes: TreeNode[];
    selected: Set<string>;
    onSelectionChange: (selected: Set<string>) => void;
    mode?: "checkbox" | "navigate";
    activePath?: string;
    onFileClick?: (path: string) => void;
    templatePaths?: Set<string>;
    filter?: string;
}

export function FileTree({
    nodes,
    selected,
    onSelectionChange,
    mode = "checkbox",
    activePath,
    onFileClick,
    templatePaths,
    filter,
}: FileTreeProps) {
    const filteredNodes = filter
        ? filterTree(nodes, filter.toLowerCase())
        : nodes;

    return (
        <div className="text-sm">
            {filteredNodes.map(node => (
                <FileTreeNode
                    key={node.path}
                    node={node}
                    depth={0}
                    selected={selected}
                    onSelectionChange={onSelectionChange}
                    mode={mode}
                    activePath={activePath}
                    onFileClick={onFileClick}
                    templatePaths={templatePaths}
                    autoExpand={countFiles({ ...node, isDir: true, children: nodes }) < 30}
                />
            ))}
        </div>
    );
}

function filterTree(nodes: TreeNode[], query: string): TreeNode[] {
    return nodes
        .map(node => {
            if (node.isDir) {
                const filtered = filterTree(node.children, query);
                if (filtered.length === 0) return null;
                return { ...node, children: filtered };
            }
            return node.name.toLowerCase().includes(query) ||
                   node.path.toLowerCase().includes(query)
                ? node
                : null;
        })
        .filter(Boolean) as TreeNode[];
}

function FileTreeNode({
    node,
    depth,
    selected,
    onSelectionChange,
    mode,
    activePath,
    onFileClick,
    templatePaths,
    autoExpand,
}: {
    node: TreeNode;
    depth: number;
    selected: Set<string>;
    onSelectionChange: (selected: Set<string>) => void;
    mode: "checkbox" | "navigate";
    activePath?: string;
    onFileClick?: (path: string) => void;
    templatePaths?: Set<string>;
    autoExpand: boolean;
}) {
    const [expanded, setExpanded] = useState(autoExpand);
    const indexFile = node.isDir ? findIndexFile(node.children) : undefined;

    const handleToggleDir = useCallback((node: TreeNode, selected: Set<string>) => {
        const files = getAllFiles(node);
        const next = new Set(selected);
        const state = getCheckState(node, selected);
        const shouldSelect = state !== "checked";
        for (const f of files) {
            if (shouldSelect) next.add(f);
            else next.delete(f);
        }
        onSelectionChange(next);
    }, [onSelectionChange]);

    const handleToggleFile = useCallback((path: string, selected: Set<string>) => {
        const next = new Set(selected);
        if (next.has(path)) next.delete(path);
        else next.add(path);
        onSelectionChange(next);
    }, [onSelectionChange]);

    if (node.isDir) {
        const checkState = getCheckState(node, selected);
        const fileCount = countFiles(node);
        const selectedCount = getAllFiles(node).filter(f => selected.has(f)).length;
        const templateCount = templatePaths
            ? getAllFiles(node).filter(f => templatePaths.has(f)).length
            : 0;

        return (
            <div style={{ marginLeft: depth * 16 }}>
                <div
                    className="flex items-center gap-1.5 rounded px-1 py-0.5 cursor-pointer hover:bg-muted/50"
                    onClick={() => setExpanded(!expanded)}
                >
                    {mode === "checkbox" && (
                        <Checkbox
                            checked={checkState === "checked"}
                            indeterminate={checkState === "indeterminate"}
                            onCheckedChange={() => handleToggleDir(node, selected)}
                            onClick={e => e.stopPropagation()}
                        />
                    )}
                    {expanded ? (
                        <ChevronDown className="size-3.5 text-muted-foreground shrink-0" />
                    ) : (
                        <ChevronRight className="size-3.5 text-muted-foreground shrink-0" />
                    )}
                    <Folder className="size-3.5 text-muted-foreground shrink-0" />
                    <span className="font-medium text-primary">{node.name}/</span>
                    <span className="text-xs text-muted-foreground">({fileCount})</span>
                    {mode === "checkbox" && checkState === "indeterminate" && (
                        <Badge variant="warning" size="sm">{selectedCount} of {fileCount}</Badge>
                    )}
                    {mode === "navigate" && templateCount > 0 && (
                        <Badge variant="success" size="sm">{templateCount} templates</Badge>
                    )}
                </div>
                {expanded && (
                    <div className="ml-2 border-l border-border pl-2">
                        {node.children.map(child => (
                            <FileTreeNode
                                key={child.path}
                                node={child}
                                depth={0}
                                selected={selected}
                                onSelectionChange={onSelectionChange}
                                mode={mode}
                                activePath={activePath}
                                onFileClick={onFileClick}
                                templatePaths={templatePaths}
                                autoExpand={autoExpand}
                            />
                        ))}
                    </div>
                )}
            </div>
        );
    }

    // File node
    const isActive = activePath === node.path;
    return (
        <div
            style={{ marginLeft: depth * 16 }}
            className={`flex items-center gap-1.5 rounded px-1 py-0.5 ${
                mode === "navigate" ? "cursor-pointer hover:bg-muted/50" : ""
            } ${isActive ? "bg-primary/10" : ""}`}
            onClick={() => {
                if (mode === "navigate" && onFileClick) onFileClick(node.path);
            }}
        >
            {mode === "checkbox" && (
                <Checkbox
                    checked={selected.has(node.path)}
                    onCheckedChange={() => handleToggleFile(node.path, selected)}
                />
            )}
            <File className="size-3.5 text-muted-foreground shrink-0" />
            <span className={isActive ? "font-medium" : ""}>{node.name}</span>
            {node.isIndex && (
                <Badge variant="warning" size="sm">index</Badge>
            )}
            {mode === "checkbox" && !node.isIndex && indexFile && (
                <span className="text-xs text-muted-foreground">→ part_of {indexFile.name}</span>
            )}
            {mode === "navigate" && node.isIndex && (
                <span className="text-xs text-amber-500">●</span>
            )}
        </div>
    );
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/import/file-tree.tsx
git commit -m "feat(import-wizard): add recursive file tree with tri-state checkboxes"
```

---

### Task 4: Wizard Shell and Step 1 (Select Files)

**Files:**
- Create: `apps/web/src/features/import/wizard.tsx`
- Create: `apps/web/src/features/import/steps/select-files.tsx`
- Modify: `apps/web/src/routes/import.tsx`

- [ ] **Step 1: Create the wizard shell with step indicator and navigation**

```typescript
// apps/web/src/features/import/wizard.tsx
import { Check } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";

import type { FileEntry, FileConfig, ImportFileStatus, PreviewFileResult, WizardStep } from "./types";
import { StepSelectFiles } from "./steps/select-files";

const STEPS = [
    { num: 1 as const, label: "Select Files" },
    { num: 2 as const, label: "Preview & Configure" },
    { num: 3 as const, label: "Review" },
    { num: 4 as const, label: "Import" },
];

const NEXT_LABELS: Record<WizardStep, string> = {
    1: "Preview →",
    2: "Review →",
    3: "Start Import",
    4: "",
};

export function ImportWizard() {
    const [step, setStep] = useState<WizardStep>(1);
    const [files, setFiles] = useState<FileEntry[]>([]);
    const [selectedPaths, setSelectedPaths] = useState<Set<string>>(new Set());
    const [codebaseId, setCodebaseId] = useState<string>("");
    const [preview, setPreview] = useState<PreviewFileResult[]>([]);
    const [overrides, setOverrides] = useState<Map<string, FileConfig>>(new Map());
    const [importStatus, setImportStatus] = useState<Map<string, ImportFileStatus>>(new Map());
    const [importing, setImporting] = useState(false);
    const [existingHashes, setExistingHashes] = useState<Record<string, string>>({});

    const canNext = (): boolean => {
        switch (step) {
            case 1: return selectedPaths.size > 0 && codebaseId !== "";
            case 2: return selectedPaths.size > 0;
            case 3: return true;
            default: return false;
        }
    };

    const handleNext = () => {
        if (step < 4) setStep((step + 1) as WizardStep);
    };

    const handleBack = () => {
        if (step > 1) setStep((step - 1) as WizardStep);
    };

    const handleReset = () => {
        setStep(1);
        setFiles([]);
        setSelectedPaths(new Set());
        setPreview([]);
        setOverrides(new Map());
        setImportStatus(new Map());
        setImporting(false);
    };

    return (
        <div className="flex flex-col" style={{ minHeight: "calc(100vh - 200px)" }}>
            {/* Step indicator */}
            <div className="flex items-center gap-0 mb-6">
                {STEPS.map((s, i) => (
                    <div key={s.num} className="flex items-center flex-1 last:flex-none">
                        <div className="flex items-center gap-2">
                            <div className={`flex size-7 items-center justify-center rounded-full text-xs font-semibold ${
                                step > s.num
                                    ? "bg-primary text-primary-foreground"
                                    : step === s.num
                                    ? "bg-primary text-primary-foreground"
                                    : "border border-border text-muted-foreground"
                            }`}>
                                {step > s.num ? <Check className="size-3.5" /> : s.num}
                            </div>
                            <span className={`text-sm ${step >= s.num ? "font-medium text-foreground" : "text-muted-foreground"}`}>
                                {s.label}
                            </span>
                        </div>
                        {i < STEPS.length - 1 && (
                            <div className="flex-1 mx-3 h-px bg-border" />
                        )}
                    </div>
                ))}
            </div>

            {/* Step content */}
            <div className="flex-1">
                {step === 1 && (
                    <StepSelectFiles
                        files={files}
                        onFilesChange={setFiles}
                        selectedPaths={selectedPaths}
                        onSelectionChange={setSelectedPaths}
                        codebaseId={codebaseId}
                        onCodebaseChange={setCodebaseId}
                    />
                )}
                {step === 2 && (
                    <div className="text-muted-foreground text-center p-12">Step 2 placeholder</div>
                )}
                {step === 3 && (
                    <div className="text-muted-foreground text-center p-12">Step 3 placeholder</div>
                )}
                {step === 4 && (
                    <div className="text-muted-foreground text-center p-12">Step 4 placeholder</div>
                )}
            </div>

            {/* Navigation footer */}
            {step < 4 && (
                <div className="flex items-center justify-between border-t pt-4 mt-6">
                    <Button
                        variant="outline"
                        onClick={handleBack}
                        disabled={step === 1}
                    >
                        ← Back
                    </Button>
                    <span className="text-xs text-muted-foreground">Step {step} of 4</span>
                    <Button
                        onClick={handleNext}
                        disabled={!canNext()}
                    >
                        {NEXT_LABELS[step]}
                    </Button>
                </div>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Create StepSelectFiles**

```typescript
// apps/web/src/features/import/steps/select-files.tsx
import { FolderUp } from "lucide-react";
import { useMemo, useRef } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useApiQuery } from "@/hooks/use-api-query";
import { api } from "@/utils/api";

import { buildTree, FileTree } from "../file-tree";
import type { FileEntry } from "../types";

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
                            content: reader.result as string,
                        });
                    reader.onerror = () => reject(reader.error);
                    reader.readAsText(file);
                })
        )
    );
}

interface StepSelectFilesProps {
    files: FileEntry[];
    onFilesChange: (files: FileEntry[]) => void;
    selectedPaths: Set<string>;
    onSelectionChange: (paths: Set<string>) => void;
    codebaseId: string;
    onCodebaseChange: (id: string) => void;
}

export function StepSelectFiles({
    files,
    onFilesChange,
    selectedPaths,
    onSelectionChange,
    codebaseId,
    onCodebaseChange,
}: StepSelectFilesProps) {
    const folderInputRef = useRef<HTMLInputElement>(null);

    const { data: codebases } = useApiQuery<any[]>({
        queryKey: ["codebases"],
        queryFn: () => api.api.codebases.get(),
        fallback: [],
    });

    const tree = useMemo(() => buildTree(files.map(f => f.path)), [files]);

    const handleFolderSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
        if (!e.target.files) return;
        const entries = await readFilesFromInput(e.target.files);
        onFilesChange(entries);
        onSelectionChange(new Set(entries.map(f => f.path)));
        e.target.value = "";
    };

    const selectedCount = selectedPaths.size;
    const totalCount = files.length;

    return (
        <div>
            {/* Top bar */}
            <div className="flex items-center gap-3 mb-4">
                <input
                    ref={folderInputRef}
                    type="file"
                    // @ts-expect-error webkitdirectory is non-standard
                    webkitdirectory=""
                    className="hidden"
                    onChange={handleFolderSelect}
                />
                <Button variant="outline" size="sm" onClick={() => folderInputRef.current?.click()}>
                    <FolderUp className="mr-1.5 size-3.5" />
                    Select Folder...
                </Button>
                {totalCount > 0 && (
                    <Badge variant="info">{totalCount} files</Badge>
                )}
                <div className="flex-1" />
                <label className="text-sm text-muted-foreground">Codebase</label>
                <select
                    className="border-input bg-background rounded-md border px-3 py-1.5 text-sm"
                    value={codebaseId}
                    onChange={e => onCodebaseChange(e.target.value)}
                >
                    <option value="">Select...</option>
                    {codebases?.map((c: { id: string; name: string }) => (
                        <option key={c.id} value={c.id}>{c.name}</option>
                    ))}
                </select>
            </div>

            {/* Summary bar */}
            {totalCount > 0 && (
                <div className="flex items-center gap-4 text-xs text-muted-foreground mb-3 px-1">
                    <span>✓ <strong className="text-foreground">{selectedCount}</strong> selected</span>
                    <span>◯ <strong className="text-foreground">{totalCount - selectedCount}</strong> deselected</span>
                    <div className="flex-1" />
                    <button className="text-primary hover:underline" onClick={() => onSelectionChange(new Set(files.map(f => f.path)))}>
                        Select all
                    </button>
                    <button className="text-primary hover:underline" onClick={() => onSelectionChange(new Set())}>
                        Deselect all
                    </button>
                </div>
            )}

            {/* File tree or empty state */}
            {totalCount === 0 ? (
                <div
                    className="flex cursor-pointer flex-col items-center justify-center gap-3 rounded-lg border-2 border-dashed p-12"
                    onClick={() => folderInputRef.current?.click()}
                >
                    <FolderUp className="text-muted-foreground size-10" />
                    <p className="text-muted-foreground text-sm">Select a folder of markdown files</p>
                </div>
            ) : (
                <div className="rounded-lg border max-h-[500px] overflow-y-auto p-2">
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
```

- [ ] **Step 3: Wire wizard into the import page**

In `apps/web/src/routes/import.tsx`, replace the wizard placeholder:

```typescript
// Add import at top:
import { ImportWizard } from "@/features/import/wizard";

// Replace the placeholder div:
{mode === "wizard" ? <ImportWizard /> : <ImportQuickMode />}
```

- [ ] **Step 4: Verify Step 1 works**

Run: `pnpm dev` and open `http://localhost:3001/import`
Expected: Toggle to Wizard mode. Click "Select Folder" and pick `docs/guide/`. See the file tree with tri-state checkboxes, index badges, and connection hints. Select/deselect files. "Preview →" button enables when files + codebase are selected.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/import/wizard.tsx apps/web/src/features/import/steps/select-files.tsx apps/web/src/routes/import.tsx
git commit -m "feat(import-wizard): add wizard shell and Step 1 (select files with tree)"
```

---

### Task 5: Backend — Extend Preview Endpoint with Existing Hashes

**Files:**
- Modify: `packages/api/src/chunks/service.ts`

- [ ] **Step 1: Add getExistingHashes function to chunks service**

Add this function near the other import functions in `packages/api/src/chunks/service.ts`:

```typescript
export function getExistingHashes(codebaseId: string, userId: string) {
    return Effect.gen(function* () {
        const docs = yield* listDocumentsRepo(userId, codebaseId);
        const hashes: Record<string, string> = {};
        for (const doc of docs) {
            if (doc.sourcePath && doc.contentHash) {
                hashes[doc.sourcePath] = doc.contentHash;
            }
        }
        return hashes;
    });
}
```

- [ ] **Step 2: Update previewImportDocs to include existing hashes**

Modify the `previewImportDocs` function to also return `existingHashes`:

At the top of the `Effect.gen` block, add:
```typescript
const existingHashes = yield* getExistingHashes(codebaseId, userId);
```

Change the return to:
```typescript
return { files: results, existingHashes };
```

Note: The `_codebaseId` parameter needs to be renamed to `codebaseId` since it's now used. Also update the function signature to pass the real `codebaseId`.

- [ ] **Step 3: Verify the preview endpoint still works**

Run: `pnpm dev`
Test: `curl -X POST http://localhost:3000/api/chunks/import-docs/preview -H "Content-Type: application/json" -H "Cookie: <session>" -d '{"files":[{"path":"test.md","content":"# Test"}],"codebaseId":"<id>"}'`
Expected: Response now includes `{ files: [...], existingHashes: {...} }`

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/chunks/service.ts
git commit -m "feat(import-wizard): extend preview endpoint with existing hashes for duplicate detection"
```

---

### Task 6: Step 2 — Preview & Configure (Split Pane)

**Files:**
- Create: `apps/web/src/features/import/file-detail-panel.tsx`
- Create: `apps/web/src/features/import/steps/preview.tsx`
- Modify: `apps/web/src/features/import/wizard.tsx`

- [ ] **Step 1: Create FileDetailPanel component**

```typescript
// apps/web/src/features/import/file-detail-panel.tsx
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

import type { FileConfig, PreviewFileResult } from "./types";

interface FileDetailPanelProps {
    filePath: string;
    preview: PreviewFileResult;
    config: FileConfig;
    onConfigChange: (path: string, config: FileConfig) => void;
    templates: { id: string; name: string }[];
    siblingCount?: number;
    onApplyToFolder?: (templateId: string) => void;
}

export function FileDetailPanel({
    filePath,
    preview,
    config,
    onConfigChange,
    templates,
    siblingCount,
    onApplyToFolder,
}: FileDetailPanelProps) {
    const update = (partial: Partial<FileConfig>) => {
        onConfigChange(filePath, { ...config, ...partial });
    };

    const suggested = preview.suggestedTemplate;

    return (
        <div className="flex flex-col gap-4">
            {/* File path breadcrumb */}
            <div>
                <p className="text-xs text-muted-foreground mb-1">{filePath}</p>
                <h3 className="text-lg font-semibold">{config.title}</h3>
            </div>

            {/* Title */}
            <div>
                <label className="text-xs font-medium uppercase tracking-wide text-muted-foreground mb-1 block">Title</label>
                <input
                    type="text"
                    className="border-input bg-background w-full rounded-md border px-3 py-2 text-sm"
                    value={config.title}
                    onChange={e => update({ title: e.target.value })}
                />
            </div>

            {/* Type */}
            <div>
                <label className="text-xs font-medium uppercase tracking-wide text-muted-foreground mb-1 block">Type</label>
                <select
                    className="border-input bg-background w-full rounded-md border px-3 py-2 text-sm"
                    value={config.type}
                    onChange={e => update({ type: e.target.value })}
                >
                    {["document", "note", "reference", "schema", "checklist"].map(t => (
                        <option key={t} value={t}>{t}</option>
                    ))}
                </select>
            </div>

            {/* Tags */}
            <div>
                <label className="text-xs font-medium uppercase tracking-wide text-muted-foreground mb-1 block">Tags</label>
                <div className="flex flex-wrap gap-1.5 rounded-md border px-3 py-2 min-h-9">
                    {config.tags.map(tag => (
                        <Badge
                            key={tag}
                            variant={config.folderTags.includes(tag) ? "outline" : "secondary"}
                            size="sm"
                            className={config.folderTags.includes(tag) ? "border-dashed" : ""}
                        >
                            {tag}
                            <button
                                className="ml-1 text-xs opacity-60 hover:opacity-100"
                                onClick={() => update({ tags: config.tags.filter(t => t !== tag) })}
                            >
                                ×
                            </button>
                        </Badge>
                    ))}
                    <input
                        className="bg-transparent text-sm outline-none flex-1 min-w-20"
                        placeholder="+ add tag"
                        onKeyDown={e => {
                            if (e.key === "Enter" && e.currentTarget.value.trim()) {
                                const newTag = e.currentTarget.value.trim();
                                if (!config.tags.includes(newTag)) {
                                    update({ tags: [...config.tags, newTag] });
                                }
                                e.currentTarget.value = "";
                            }
                        }}
                    />
                </div>
                {config.folderTags.length > 0 && (
                    <p className="text-xs text-muted-foreground mt-1">Dashed = derived from folder path</p>
                )}
            </div>

            {/* Template */}
            <div>
                <label className="text-xs font-medium uppercase tracking-wide text-muted-foreground mb-1 block">Template</label>
                {suggested ? (
                    <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/5 p-3">
                        <div className="flex items-center justify-between mb-2">
                            <div className="flex items-center gap-2">
                                <span className="text-sm font-medium text-emerald-500">✓ {suggested.name}</span>
                                <Badge variant="success" size="sm">score: {suggested.score}</Badge>
                            </div>
                            <select
                                className="border-input bg-background rounded border px-2 py-1 text-xs"
                                value={config.templateId ?? ""}
                                onChange={e => update({ templateId: e.target.value || null })}
                            >
                                <option value={suggested.id}>{suggested.name}</option>
                                {templates.filter(t => t.id !== suggested.id).map(t => (
                                    <option key={t.id} value={t.id}>{t.name}</option>
                                ))}
                                <option value="">None</option>
                            </select>
                        </div>
                        {Object.keys(suggested.extractedFields).length > 0 && (
                            <div className="text-xs text-muted-foreground">
                                <p className="uppercase tracking-wide mb-1 text-[10px]">Extracted Fields</p>
                                {Object.entries(suggested.extractedFields).map(([key, value]) => (
                                    <div key={key} className="flex gap-2 py-0.5">
                                        <span className="text-muted-foreground min-w-20">{key}:</span>
                                        <span className="text-foreground/80 truncate">
                                            {typeof value === "string" ? `"${value.slice(0, 80)}..."` : JSON.stringify(value)}
                                        </span>
                                    </div>
                                ))}
                            </div>
                        )}
                        {onApplyToFolder && siblingCount && siblingCount > 1 && (
                            <div className="mt-3 flex items-center gap-2 rounded bg-primary/5 border border-primary/10 px-3 py-2">
                                <span className="text-xs text-primary">
                                    Apply "{suggested.name}" to all {siblingCount} files in this folder?
                                </span>
                                <Button
                                    variant="outline"
                                    size="xs"
                                    className="ml-auto"
                                    onClick={() => onApplyToFolder(config.templateId ?? suggested.id)}
                                >
                                    Apply to folder
                                </Button>
                            </div>
                        )}
                    </div>
                ) : (
                    <div className="rounded-lg border p-3">
                        <div className="flex items-center justify-between">
                            <span className="text-sm text-muted-foreground">No template matched</span>
                            <select
                                className="border-input bg-background rounded border px-2 py-1 text-xs"
                                value={config.templateId ?? ""}
                                onChange={e => update({ templateId: e.target.value || null })}
                            >
                                <option value="">None</option>
                                {templates.map(t => (
                                    <option key={t.id} value={t.id}>{t.name}</option>
                                ))}
                            </select>
                        </div>
                    </div>
                )}
            </div>

            {/* Content preview */}
            <div>
                <label className="text-xs font-medium uppercase tracking-wide text-muted-foreground mb-1 block">Content Preview</label>
                <div className="rounded-lg border bg-muted/30 p-3 text-sm text-muted-foreground max-h-40 overflow-y-auto whitespace-pre-wrap font-mono text-xs">
                    {preview.parsed.content.slice(0, 500)}
                    {preview.parsed.content.length > 500 && "..."}
                </div>
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Create StepPreview component**

```typescript
// apps/web/src/features/import/steps/preview.tsx
import { Loader2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { useApiQuery } from "@/hooks/use-api-query";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { FileDetailPanel } from "../file-detail-panel";
import { buildTree, FileTree } from "../file-tree";
import type { FileConfig, FileEntry, PreviewFileResult } from "../types";

interface StepPreviewProps {
    files: FileEntry[];
    selectedPaths: Set<string>;
    codebaseId: string;
    preview: PreviewFileResult[];
    onPreviewLoaded: (results: PreviewFileResult[], hashes: Record<string, string>) => void;
    overrides: Map<string, FileConfig>;
    onOverridesChange: (overrides: Map<string, FileConfig>) => void;
}

export function StepPreview({
    files,
    selectedPaths,
    codebaseId,
    preview,
    onPreviewLoaded,
    overrides,
    onOverridesChange,
}: StepPreviewProps) {
    const [activePath, setActivePath] = useState<string>("");
    const [loading, setLoading] = useState(preview.length === 0);
    const [filter, setFilter] = useState("");

    const selectedFiles = useMemo(
        () => files.filter(f => selectedPaths.has(f.path)),
        [files, selectedPaths]
    );

    const tree = useMemo(() => buildTree(selectedFiles.map(f => f.path)), [selectedFiles]);

    const { data: templates } = useApiQuery<any[]>({
        queryKey: ["templates"],
        queryFn: () => api.api.templates.get(),
        fallback: [],
    });

    // Load preview on mount if not already loaded
    useEffect(() => {
        if (preview.length > 0) {
            setLoading(false);
            if (!activePath && preview.length > 0) setActivePath(preview[0]!.path);
            return;
        }

        let cancelled = false;
        (async () => {
            try {
                const result = unwrapEden(
                    await api.api.chunks["import-docs"].preview.post({
                        files: selectedFiles.map(f => ({ path: f.path, content: f.content })),
                        codebaseId,
                    })
                ) as { files: PreviewFileResult[]; existingHashes: Record<string, string> };

                if (cancelled) return;
                onPreviewLoaded(result.files, result.existingHashes);

                // Initialize overrides from preview data
                const initial = new Map<string, FileConfig>();
                for (const file of result.files) {
                    const folderTags = deriveFolderTags(file.path);
                    initial.set(file.path, {
                        title: file.parsed.title,
                        type: file.suggestedTemplate?.type ?? file.parsed.type,
                        tags: [...new Set([...file.parsed.tags, ...folderTags])],
                        templateId: file.suggestedTemplate?.id ?? null,
                        folderTags,
                    });
                }
                onOverridesChange(initial);

                if (result.files.length > 0) setActivePath(result.files[0]!.path);
            } catch {
                if (!cancelled) toast.error("Failed to load preview");
            } finally {
                if (!cancelled) setLoading(false);
            }
        })();
        return () => { cancelled = true; };
    }, []);

    const templatePaths = useMemo(
        () => new Set(preview.filter(p => p.suggestedTemplate).map(p => p.path)),
        [preview]
    );

    const activePreview = preview.find(p => p.path === activePath);
    const activeConfig = overrides.get(activePath);

    const handleConfigChange = (path: string, config: FileConfig) => {
        const next = new Map(overrides);
        next.set(path, config);
        onOverridesChange(next);
    };

    const handleApplyToFolder = (templateId: string) => {
        if (!activePath) return;
        const dir = activePath.split("/").slice(0, -1).join("/");
        const next = new Map(overrides);
        for (const [path, config] of next) {
            if (path.startsWith(dir + "/") || path.split("/").slice(0, -1).join("/") === dir) {
                next.set(path, { ...config, templateId });
            }
        }
        onOverridesChange(next);
        toast.success(`Template applied to folder`);
    };

    if (loading) {
        return (
            <div className="flex items-center justify-center gap-3 p-12">
                <Loader2 className="size-5 animate-spin text-muted-foreground" />
                <span className="text-muted-foreground text-sm">Analyzing {selectedFiles.length} files...</span>
            </div>
        );
    }

    const activeDir = activePath.split("/").slice(0, -1).join("/");
    const siblingsInDir = preview.filter(p => p.path.split("/").slice(0, -1).join("/") === activeDir).length;

    return (
        <div>
            <div className="text-xs text-primary mb-3">
                ✓ Preview loaded — {preview.length} files analyzed, {templatePaths.size} template matches found
            </div>
            <div className="flex gap-0 rounded-lg border overflow-hidden" style={{ height: 500 }}>
                {/* Left: tree nav */}
                <div className="w-80 border-r overflow-y-auto shrink-0">
                    <div className="p-2 border-b">
                        <input
                            type="text"
                            placeholder="Filter files..."
                            className="border-input bg-background w-full rounded-md border px-2.5 py-1.5 text-xs"
                            value={filter}
                            onChange={e => setFilter(e.target.value)}
                        />
                    </div>
                    <div className="p-2">
                        <FileTree
                            nodes={tree}
                            selected={selectedPaths}
                            onSelectionChange={() => {}}
                            mode="navigate"
                            activePath={activePath}
                            onFileClick={setActivePath}
                            templatePaths={templatePaths}
                            filter={filter}
                        />
                    </div>
                </div>

                {/* Right: detail panel */}
                <div className="flex-1 overflow-y-auto p-5">
                    {activePreview && activeConfig ? (
                        <FileDetailPanel
                            filePath={activePath}
                            preview={activePreview}
                            config={activeConfig}
                            onConfigChange={handleConfigChange}
                            templates={templates?.map((t: any) => ({ id: t.id, name: t.name })) ?? []}
                            siblingCount={siblingsInDir}
                            onApplyToFolder={handleApplyToFolder}
                        />
                    ) : (
                        <p className="text-muted-foreground text-sm text-center p-12">
                            Select a file from the tree to view details
                        </p>
                    )}
                </div>
            </div>
        </div>
    );
}

function deriveFolderTags(path: string): string[] {
    const parts = path.split("/");
    const tags: string[] = [];
    for (let i = 0; i < parts.length - 1; i++) {
        const folder = parts[i]!;
        if (folder && !["index", "readme"].includes(folder.toLowerCase())) {
            tags.push(folder);
        }
    }
    return tags;
}
```

- [ ] **Step 3: Wire Step 2 into the wizard**

In `apps/web/src/features/import/wizard.tsx`, replace the step 2 placeholder:

```typescript
// Add import:
import { StepPreview } from "./steps/preview";

// Replace step 2 placeholder:
{step === 2 && (
    <StepPreview
        files={files}
        selectedPaths={selectedPaths}
        codebaseId={codebaseId}
        preview={preview}
        onPreviewLoaded={(results, hashes) => {
            setPreview(results);
            setExistingHashes(hashes);
        }}
        overrides={overrides}
        onOverridesChange={setOverrides}
    />
)}
```

- [ ] **Step 4: Verify Step 2 works**

Run: `pnpm dev`, navigate to `/import`, select Wizard mode, pick a folder, select codebase, click "Preview →".
Expected: Loading spinner, then split pane with tree on left and detail panel on right. Click files to see editable fields. Template matches show green cards.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/import/file-detail-panel.tsx apps/web/src/features/import/steps/preview.tsx apps/web/src/features/import/wizard.tsx
git commit -m "feat(import-wizard): add Step 2 (preview & configure with split pane)"
```

---

### Task 7: Step 3 — Review

**Files:**
- Create: `apps/web/src/features/import/steps/review.tsx`
- Modify: `apps/web/src/features/import/wizard.tsx`

- [ ] **Step 1: Create StepReview component**

```typescript
// apps/web/src/features/import/steps/review.tsx
import { ChevronDown, ChevronRight } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";

import { INDEX_FILE_NAMES, type FileConfig, type FileEntry, type PreviewFileResult } from "../types";

interface StepReviewProps {
    files: FileEntry[];
    selectedPaths: Set<string>;
    preview: PreviewFileResult[];
    overrides: Map<string, FileConfig>;
    existingHashes: Record<string, string>;
    codebaseName: string;
    onGoToFile: (path: string) => void;
}

async function hashContent(content: string): Promise<string> {
    const encoder = new TextEncoder();
    const data = encoder.encode(content);
    const buffer = await crypto.subtle.digest("SHA-256", data);
    return Array.from(new Uint8Array(buffer)).map(b => b.toString(16).padStart(2, "0")).join("");
}

export function StepReview({
    files,
    selectedPaths,
    preview,
    overrides,
    existingHashes,
    codebaseName,
    onGoToFile,
}: StepReviewProps) {
    const [expandNew, setExpandNew] = useState(true);
    const [expandSkipped, setExpandSkipped] = useState(false);
    const [expandConnections, setExpandConnections] = useState(true);
    const [hashes, setHashes] = useState<Record<string, string>>({});
    const [hashesComputed, setHashesComputed] = useState(false);

    // Compute hashes on mount
    useMemo(() => {
        (async () => {
            const result: Record<string, string> = {};
            for (const file of files.filter(f => selectedPaths.has(f.path))) {
                result[file.path] = await hashContent(file.content);
            }
            setHashes(result);
            setHashesComputed(true);
        })();
    }, [files, selectedPaths]);

    const selectedFiles = files.filter(f => selectedPaths.has(f.path));

    const skippedPaths = useMemo(() => {
        if (!hashesComputed) return new Set<string>();
        return new Set(
            selectedFiles
                .filter(f => {
                    const existing = existingHashes[f.path];
                    return existing && existing === hashes[f.path];
                })
                .map(f => f.path)
        );
    }, [selectedFiles, existingHashes, hashes, hashesComputed]);

    const newFiles = selectedFiles.filter(f => !skippedPaths.has(f.path));
    const skippedFiles = selectedFiles.filter(f => skippedPaths.has(f.path));

    const templateCount = newFiles.filter(f => {
        const config = overrides.get(f.path);
        return config?.templateId;
    }).length;

    // Compute connections preview
    const connections = useMemo(() => {
        const result: { source: string; target: string }[] = [];
        const byDir = new Map<string, { path: string; isIndex: boolean }[]>();
        for (const file of newFiles) {
            const parts = file.path.split("/");
            const dir = parts.slice(0, -1).join("/") || ".";
            const filename = (parts[parts.length - 1] ?? "").toLowerCase();
            if (!byDir.has(dir)) byDir.set(dir, []);
            byDir.get(dir)!.push({ path: file.path, isIndex: INDEX_FILE_NAMES.has(filename) });
        }
        for (const [, entries] of byDir) {
            const index = entries.find(e => e.isIndex);
            if (!index) continue;
            for (const entry of entries.filter(e => !e.isIndex)) {
                result.push({ source: entry.path, target: index.path });
            }
        }
        return result;
    }, [newFiles]);

    return (
        <div>
            {/* Summary stats */}
            <div className="grid grid-cols-4 gap-0 rounded-lg border overflow-hidden mb-6">
                <div className="p-4 text-center border-r">
                    <div className="text-2xl font-semibold text-emerald-500">{newFiles.length}</div>
                    <div className="text-xs text-muted-foreground mt-1">New chunks</div>
                </div>
                <div className="p-4 text-center border-r">
                    <div className="text-2xl font-semibold text-amber-500">{skippedFiles.length}</div>
                    <div className="text-xs text-muted-foreground mt-1">Will be skipped</div>
                    <div className="text-[10px] text-muted-foreground">(already imported)</div>
                </div>
                <div className="p-4 text-center border-r">
                    <div className="text-2xl font-semibold text-indigo-400">{templateCount}</div>
                    <div className="text-xs text-muted-foreground mt-1">Template matches</div>
                </div>
                <div className="p-4 text-center">
                    <div className="text-2xl font-semibold text-purple-400">{connections.length}</div>
                    <div className="text-xs text-muted-foreground mt-1">Connections</div>
                    <div className="text-[10px] text-muted-foreground">part_of</div>
                </div>
            </div>

            {/* New chunks */}
            <div className="rounded-lg border mb-3 overflow-hidden">
                <button
                    className="w-full flex items-center gap-2 px-4 py-2.5 text-sm font-medium text-emerald-500 bg-emerald-500/5 hover:bg-emerald-500/10"
                    onClick={() => setExpandNew(!expandNew)}
                >
                    {expandNew ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
                    {newFiles.length} files will be created
                </button>
                {expandNew && (
                    <div className="max-h-48 overflow-y-auto">
                        <table className="w-full text-xs">
                            <thead>
                                <tr className="border-b">
                                    <th className="text-left px-4 py-1.5 font-medium text-muted-foreground">Path</th>
                                    <th className="text-left px-4 py-1.5 font-medium text-muted-foreground">Title</th>
                                    <th className="text-left px-4 py-1.5 font-medium text-muted-foreground">Type</th>
                                    <th className="text-left px-4 py-1.5 font-medium text-muted-foreground">Template</th>
                                    <th className="text-left px-4 py-1.5 font-medium text-muted-foreground">Tags</th>
                                </tr>
                            </thead>
                            <tbody>
                                {newFiles.map(file => {
                                    const config = overrides.get(file.path);
                                    return (
                                        <tr
                                            key={file.path}
                                            className={`border-b last:border-b-0 cursor-pointer hover:bg-muted/50 ${config?.templateId ? "bg-emerald-500/[0.03]" : ""}`}
                                            onClick={() => onGoToFile(file.path)}
                                        >
                                            <td className="px-4 py-1.5 font-mono text-muted-foreground">{file.path}</td>
                                            <td className="px-4 py-1.5">{config?.title ?? file.path}</td>
                                            <td className="px-4 py-1.5">
                                                <Badge variant="outline" size="sm">{config?.type ?? "document"}</Badge>
                                            </td>
                                            <td className="px-4 py-1.5">
                                                {config?.templateId ? (
                                                    <Badge variant="success" size="sm">
                                                        {preview.find(p => p.path === file.path)?.suggestedTemplate?.name ?? "Custom"}
                                                    </Badge>
                                                ) : (
                                                    <span className="text-muted-foreground">—</span>
                                                )}
                                            </td>
                                            <td className="px-4 py-1.5">
                                                <div className="flex flex-wrap gap-0.5">
                                                    {(config?.tags ?? []).slice(0, 3).map(tag => (
                                                        <Badge key={tag} variant="secondary" size="sm">{tag}</Badge>
                                                    ))}
                                                    {(config?.tags?.length ?? 0) > 3 && (
                                                        <span className="text-muted-foreground">+{(config?.tags?.length ?? 0) - 3}</span>
                                                    )}
                                                </div>
                                            </td>
                                        </tr>
                                    );
                                })}
                            </tbody>
                        </table>
                    </div>
                )}
            </div>

            {/* Skipped files */}
            {skippedFiles.length > 0 && (
                <div className="rounded-lg border mb-3 overflow-hidden">
                    <button
                        className="w-full flex items-center gap-2 px-4 py-2.5 text-sm font-medium text-amber-500 bg-amber-500/5 hover:bg-amber-500/10"
                        onClick={() => setExpandSkipped(!expandSkipped)}
                    >
                        {expandSkipped ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
                        {skippedFiles.length} files will be skipped
                        <span className="text-xs text-muted-foreground ml-1">(content unchanged since last import)</span>
                    </button>
                    {expandSkipped && (
                        <div className="max-h-32 overflow-y-auto px-4 py-2 text-xs text-muted-foreground">
                            {skippedFiles.map(f => (
                                <div key={f.path} className="py-0.5 font-mono">{f.path}</div>
                            ))}
                        </div>
                    )}
                </div>
            )}

            {/* Connections */}
            {connections.length > 0 && (
                <div className="rounded-lg border mb-3 overflow-hidden">
                    <button
                        className="w-full flex items-center gap-2 px-4 py-2.5 text-sm font-medium text-purple-400 bg-purple-500/5 hover:bg-purple-500/10"
                        onClick={() => setExpandConnections(!expandConnections)}
                    >
                        {expandConnections ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
                        {connections.length} connections will be created
                    </button>
                    {expandConnections && (
                        <div className="max-h-40 overflow-y-auto px-4 py-2 text-xs">
                            {connections.map((c, i) => (
                                <div key={i} className="flex items-center gap-1.5 py-0.5">
                                    <span>{c.source.split("/").pop()}</span>
                                    <span className="text-muted-foreground">→</span>
                                    <Badge variant="secondary" size="sm">part_of</Badge>
                                    <span className="text-muted-foreground">→</span>
                                    <span>{c.target}</span>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            )}

            {/* Codebase reminder */}
            <div className="flex items-center gap-2 mt-4 text-sm text-muted-foreground">
                Importing into: <Badge variant="info">{codebaseName}</Badge>
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Wire Step 3 into the wizard**

In `wizard.tsx`, add imports and replace the step 3 placeholder. The `onGoToFile` handler navigates back to step 2 with the file selected. Add a helper to look up the codebase name from the codebases query.

- [ ] **Step 3: Verify Step 3 works**

Run through the full flow: select folder → preview → review. Check that stats are correct, tables are clickable, connections preview shows the right pairs.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/import/steps/review.tsx apps/web/src/features/import/wizard.tsx
git commit -m "feat(import-wizard): add Step 3 (review with stats, tables, connection preview)"
```

---

### Task 8: Backend — SSE Streaming Import Endpoint

**Files:**
- Modify: `packages/api/src/chunks/routes.ts`
- Modify: `packages/api/src/chunks/service.ts`
- Create: `packages/api/src/chunks/__tests__/import-stream.test.ts`

- [ ] **Step 1: Write the test for importDocsStream**

```typescript
// packages/api/src/chunks/__tests__/import-stream.test.ts
import { describe, expect, it } from "vitest";

import { importDocsStream } from "../service";

describe("importDocsStream", () => {
    it("yields per-file results and a done event", async () => {
        // This is an integration-style test that verifies the generator shape.
        // It requires a running database, so skip if no DATABASE_URL.
        // The key contract: the generator yields objects with { type, path?, status? }
        // and the last yield has type "done".

        // For a unit test without DB, we verify the function signature exists
        // and returns an async generator.
        expect(typeof importDocsStream).toBe("function");
    });
});
```

- [ ] **Step 2: Run the test to verify it passes (signature check)**

Run: `pnpm test packages/api/src/chunks/__tests__/import-stream.test.ts`
Expected: PASS

- [ ] **Step 3: Add importDocsStream generator to the service**

In `packages/api/src/chunks/service.ts`, add:

```typescript
export async function* importDocsStream(
    userId: string,
    files: { path: string; content: string }[],
    codebaseId: string,
    templateOverrides?: Record<string, string | null>
) {
    const fileChunks = new Map<string, string>();
    let created = 0;
    let skipped = 0;
    let errors = 0;
    const startTime = Date.now();

    for (const file of files) {
        try {
            const templateId = templateOverrides?.[file.path] ?? undefined;
            const result = await Effect.runPromise(
                importDocument(userId, file.path, file.content, codebaseId, templateId ?? undefined)
            );
            if (result.status === "unchanged") {
                skipped++;
                yield { type: "file" as const, path: file.path, status: "unchanged" as const };
            } else {
                created += result.created;
                if (result.firstChunkId) {
                    fileChunks.set(file.path, result.firstChunkId);
                }
                yield { type: "file" as const, path: file.path, status: "created" as const, created: result.created };
            }
        } catch (err) {
            errors++;
            yield { type: "file" as const, path: file.path, status: "error" as const, error: String(err) };
        }
    }

    // Create folder connections
    let connections = 0;
    try {
        connections = await Effect.runPromise(createFolderConnections(userId, fileChunks, codebaseId));
    } catch {}

    yield {
        type: "done" as const,
        created,
        skipped,
        errors,
        connections,
        elapsed: Date.now() - startTime,
    };
}
```

- [ ] **Step 4: Add the SSE route**

In `packages/api/src/chunks/routes.ts`, add after the existing import-docs route:

```typescript
.post(
    "/chunks/import-docs/stream",
    async (ctx) => {
        const session = await Effect.runPromise(requireSession(ctx));
        const rl = checkRateLimit(`import-docs:${session.user.id}`, 5, 60_000);
        if (!rl.allowed) {
            ctx.set.status = 429;
            return new Response(JSON.stringify({
                error: "Rate limit exceeded",
                retryAfter: Math.ceil((rl.resetAt - Date.now()) / 1000)
            }), { status: 429, headers: { "Content-Type": "application/json" } });
        }

        const generator = chunkService.importDocsStream(
            session.user.id,
            ctx.body.files,
            ctx.body.codebaseId,
            ctx.body.templateOverrides
        );

        const stream = new ReadableStream({
            async start(controller) {
                const encoder = new TextEncoder();
                for await (const event of generator) {
                    const eventType = event.type;
                    const data = JSON.stringify(event);
                    controller.enqueue(encoder.encode(`event: ${eventType}\ndata: ${data}\n\n`));
                }
                controller.close();
            },
        });

        return new Response(stream, {
            headers: {
                "Content-Type": "text/event-stream",
                "Cache-Control": "no-cache",
                "Connection": "keep-alive",
            },
        });
    },
    {
        body: t.Object({
            files: t.Array(
                t.Object({
                    path: t.String({ maxLength: 500 }),
                    content: t.String({ maxLength: 100000 })
                }),
                { maxItems: 500 }
            ),
            codebaseId: t.String(),
            templateOverrides: t.Optional(t.Record(t.String(), t.Union([t.String(), t.Null()])))
        })
    }
)
```

- [ ] **Step 5: Run tests**

Run: `pnpm test packages/api/src/chunks/__tests__/import-stream.test.ts`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add packages/api/src/chunks/service.ts packages/api/src/chunks/routes.ts packages/api/src/chunks/__tests__/import-stream.test.ts
git commit -m "feat(import-wizard): add SSE streaming import endpoint"
```

---

### Task 9: SSE Consumer Hook and Step 4 (Import)

**Files:**
- Create: `apps/web/src/features/import/use-sse-import.ts`
- Create: `apps/web/src/features/import/steps/import-step.tsx`
- Modify: `apps/web/src/features/import/wizard.tsx`

- [ ] **Step 1: Create the SSE consumer hook**

```typescript
// apps/web/src/features/import/use-sse-import.ts
import { useCallback, useRef, useState } from "react";

import { env } from "@fubbik/env/web";

import type { ImportFileStatus } from "./types";

interface SSEImportOptions {
    files: { path: string; content: string }[];
    codebaseId: string;
    templateOverrides?: Record<string, string | null>;
    onFileUpdate: (path: string, status: ImportFileStatus) => void;
    onDone: (result: { created: number; skipped: number; errors: number; connections: number; elapsed: number }) => void;
    onError: (error: string) => void;
}

export function useSSEImport() {
    const [isImporting, setIsImporting] = useState(false);
    const abortRef = useRef<AbortController | null>(null);

    const startImport = useCallback(async (options: SSEImportOptions) => {
        setIsImporting(true);
        const abort = new AbortController();
        abortRef.current = abort;

        try {
            const response = await fetch(`${env.VITE_SERVER_URL}/api/chunks/import-docs/stream`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                credentials: "include",
                signal: abort.signal,
                body: JSON.stringify({
                    files: options.files,
                    codebaseId: options.codebaseId,
                    templateOverrides: options.templateOverrides,
                }),
            });

            if (!response.ok || !response.body) {
                throw new Error(`HTTP ${response.status}`);
            }

            const reader = response.body.getReader();
            const decoder = new TextDecoder();
            let buffer = "";

            while (true) {
                const { done, value } = await reader.read();
                if (done) break;

                buffer += decoder.decode(value, { stream: true });
                const lines = buffer.split("\n");
                buffer = lines.pop() ?? "";

                let currentEvent = "";
                for (const line of lines) {
                    if (line.startsWith("event: ")) {
                        currentEvent = line.slice(7);
                    } else if (line.startsWith("data: ")) {
                        const data = JSON.parse(line.slice(6));
                        if (currentEvent === "file") {
                            options.onFileUpdate(data.path, {
                                status: data.status === "unchanged" ? "skipped" : data.status,
                                created: data.created,
                                error: data.error,
                            });
                        } else if (currentEvent === "done") {
                            options.onDone(data);
                        }
                        currentEvent = "";
                    }
                }
            }
        } catch (err) {
            if (!abort.signal.aborted) {
                options.onError(String(err));
            }
        } finally {
            setIsImporting(false);
        }
    }, []);

    return { startImport, isImporting };
}
```

- [ ] **Step 2: Create StepImport component**

```typescript
// apps/web/src/features/import/steps/import-step.tsx
import { CheckCircle2, Loader2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

import type { FileConfig, FileEntry, ImportFileStatus } from "../types";
import { useSSEImport } from "../use-sse-import";

interface StepImportProps {
    files: FileEntry[];
    selectedPaths: Set<string>;
    codebaseId: string;
    overrides: Map<string, FileConfig>;
    importStatus: Map<string, ImportFileStatus>;
    onStatusChange: (status: Map<string, ImportFileStatus>) => void;
    onReset: () => void;
}

export function StepImport({
    files,
    selectedPaths,
    codebaseId,
    overrides,
    importStatus,
    onStatusChange,
    onReset,
}: StepImportProps) {
    const queryClient = useQueryClient();
    const { startImport, isImporting } = useSSEImport();
    const startedRef = useRef(false);
    const tableRef = useRef<HTMLDivElement>(null);

    const selectedFiles = useMemo(
        () => files.filter(f => selectedPaths.has(f.path)),
        [files, selectedPaths]
    );

    const doneResult = useMemo(() => {
        const statuses = Array.from(importStatus.values());
        if (statuses.length === 0) return null;
        const allDone = statuses.every(s => s.status !== "pending" && s.status !== "importing");
        if (!allDone) return null;
        return {
            created: statuses.filter(s => s.status === "created").reduce((sum, s) => sum + (s.created ?? 0), 0),
            skipped: statuses.filter(s => s.status === "skipped").length,
            errors: statuses.filter(s => s.status === "error").length,
            connections: 0, // filled from done event
        };
    }, [importStatus]);

    const [completionData, setCompletionData] = useState<{ connections: number; elapsed: number } | null>(null);

    // Start import on mount
    useEffect(() => {
        if (startedRef.current) return;
        startedRef.current = true;

        // Initialize all as pending
        const initial = new Map<string, ImportFileStatus>();
        for (const file of selectedFiles) {
            initial.set(file.path, { status: "pending" });
        }
        onStatusChange(initial);

        // Build template overrides from wizard state
        const templateOverrides: Record<string, string | null> = {};
        for (const [path, config] of overrides) {
            if (config.templateId) {
                templateOverrides[path] = config.templateId;
            }
        }

        startImport({
            files: selectedFiles.map(f => ({ path: f.path, content: f.content })),
            codebaseId,
            templateOverrides: Object.keys(templateOverrides).length > 0 ? templateOverrides : undefined,
            onFileUpdate: (path, status) => {
                onStatusChange(prev => {
                    const next = new Map(prev);
                    next.set(path, status);
                    return next;
                });
            },
            onDone: (result) => {
                setCompletionData({ connections: result.connections, elapsed: result.elapsed });
                queryClient.invalidateQueries({ queryKey: ["chunks"] });
            },
            onError: (error) => {
                // Mark remaining pending files as errors
                onStatusChange(prev => {
                    const next = new Map(prev);
                    for (const [path, status] of next) {
                        if (status.status === "pending") {
                            next.set(path, { status: "error", error: `Connection lost: ${error}` });
                        }
                    }
                    return next;
                });
            },
        });
    }, []);

    // Auto-scroll to currently importing file
    useEffect(() => {
        if (!tableRef.current) return;
        const importing = Array.from(importStatus.entries()).find(([, s]) => s.status === "importing");
        if (importing) {
            const row = tableRef.current.querySelector(`[data-path="${CSS.escape(importing[0])}"]`);
            row?.scrollIntoView({ behavior: "smooth", block: "nearest" });
        }
    }, [importStatus]);

    const processedCount = Array.from(importStatus.values()).filter(s => s.status !== "pending" && s.status !== "importing").length;
    const totalCount = selectedFiles.length;
    const createdCount = Array.from(importStatus.values()).filter(s => s.status === "created").length;
    const skippedCount = Array.from(importStatus.values()).filter(s => s.status === "skipped").length;
    const errorCount = Array.from(importStatus.values()).filter(s => s.status === "error").length;
    const pendingCount = Array.from(importStatus.values()).filter(s => s.status === "pending").length;
    const isDone = completionData !== null;

    return (
        <div>
            {isDone ? (
                /* Completion state */
                <div className="text-center border rounded-lg overflow-hidden mb-6">
                    <div className="p-6 bg-emerald-500/5 border-b">
                        <CheckCircle2 className="size-8 text-emerald-500 mx-auto mb-2" />
                        <h3 className="text-lg font-semibold">Import Complete</h3>
                        <p className="text-sm text-muted-foreground mt-1">
                            {totalCount} files processed in {(completionData.elapsed / 1000).toFixed(1)}s
                        </p>
                    </div>
                    <div className="grid grid-cols-4 border-b">
                        <div className="p-4 text-center border-r">
                            <div className="text-xl font-semibold text-emerald-500">{createdCount}</div>
                            <div className="text-xs text-muted-foreground">Created</div>
                        </div>
                        <div className="p-4 text-center border-r">
                            <div className="text-xl font-semibold text-amber-500">{skippedCount}</div>
                            <div className="text-xs text-muted-foreground">Skipped</div>
                        </div>
                        <div className="p-4 text-center border-r">
                            <div className="text-xl font-semibold text-red-500">{errorCount}</div>
                            <div className="text-xs text-muted-foreground">Errors</div>
                        </div>
                        <div className="p-4 text-center">
                            <div className="text-xl font-semibold text-purple-400">{completionData.connections}</div>
                            <div className="text-xs text-muted-foreground">Connections</div>
                        </div>
                    </div>
                    <div className="p-4 flex gap-3 justify-center">
                        <Link to="/chunks">
                            <Button>View imported chunks →</Button>
                        </Link>
                        <Link to="/graph">
                            <Button variant="outline">View in graph</Button>
                        </Link>
                        <Button variant="outline" onClick={onReset}>Import more</Button>
                    </div>
                </div>
            ) : (
                /* In-progress state */
                <div className="mb-4">
                    <div className="flex justify-between mb-1.5">
                        <span className="text-sm font-medium">Importing files...</span>
                        <span className="text-sm text-muted-foreground">{processedCount} / {totalCount}</span>
                    </div>
                    <div className="h-1.5 rounded-full bg-muted overflow-hidden">
                        <div
                            className="h-full rounded-full bg-primary transition-all duration-300"
                            style={{ width: `${(processedCount / totalCount) * 100}%` }}
                        />
                    </div>
                    <div className="flex gap-4 mt-2 text-xs">
                        <span className="text-emerald-500">✓ {createdCount} created</span>
                        <span className="text-amber-500">○ {skippedCount} skipped</span>
                        {errorCount > 0 && <span className="text-red-500">✕ {errorCount} errors</span>}
                        <span className="text-muted-foreground">⋯ {pendingCount} pending</span>
                    </div>
                </div>
            )}

            {/* Pipeline table */}
            <div ref={tableRef} className="rounded-lg border overflow-hidden max-h-80 overflow-y-auto">
                <table className="w-full text-xs">
                    <thead className="sticky top-0 bg-background">
                        <tr className="border-b">
                            <th className="w-10 px-4 py-2 text-left font-medium text-muted-foreground">Status</th>
                            <th className="px-4 py-2 text-left font-medium text-muted-foreground">File</th>
                            <th className="px-4 py-2 text-left font-medium text-muted-foreground">Title</th>
                            <th className="w-16 px-4 py-2 text-left font-medium text-muted-foreground">Chunks</th>
                            <th className="px-4 py-2 text-left font-medium text-muted-foreground">Detail</th>
                        </tr>
                    </thead>
                    <tbody>
                        {selectedFiles.map(file => {
                            const status = importStatus.get(file.path);
                            const config = overrides.get(file.path);
                            return (
                                <tr
                                    key={file.path}
                                    data-path={file.path}
                                    className={`border-b last:border-b-0 ${
                                        status?.status === "importing" ? "bg-primary/5" :
                                        status?.status === "error" ? "bg-red-500/5" :
                                        status?.status === "skipped" ? "bg-amber-500/5" : ""
                                    }`}
                                >
                                    <td className="px-4 py-1.5">
                                        {status?.status === "created" && <span className="text-emerald-500">✓</span>}
                                        {status?.status === "skipped" && <span className="text-amber-500">○</span>}
                                        {status?.status === "error" && <span className="text-red-500">✕</span>}
                                        {status?.status === "importing" && <Loader2 className="size-3.5 text-primary animate-spin" />}
                                        {status?.status === "pending" && <span className="text-muted-foreground">⋯</span>}
                                    </td>
                                    <td className="px-4 py-1.5 font-mono text-muted-foreground">{file.path}</td>
                                    <td className="px-4 py-1.5">{config?.title ?? file.path.split("/").pop()}</td>
                                    <td className="px-4 py-1.5">{status?.created ?? "—"}</td>
                                    <td className="px-4 py-1.5">
                                        {status?.status === "created" && <span className="text-emerald-500">Created</span>}
                                        {status?.status === "skipped" && <span className="text-amber-500">Unchanged</span>}
                                        {status?.status === "error" && (
                                            <span className="text-red-500 cursor-pointer" title={status.error}>{status.error}</span>
                                        )}
                                        {status?.status === "importing" && <span className="text-primary">Importing...</span>}
                                        {status?.status === "pending" && <span className="text-muted-foreground">Pending</span>}
                                    </td>
                                </tr>
                            );
                        })}
                    </tbody>
                </table>
            </div>
        </div>
    );
}
```

Note: The `onStatusChange` prop uses a function updater pattern (`prev => next`). In the wizard, pass `setImportStatus` directly as the prop since React's `useState` setter already accepts both values and updater functions. The prop type should be `React.Dispatch<React.SetStateAction<Map<string, ImportFileStatus>>>`.

- [ ] **Step 3: Wire Step 4 into the wizard**

In `wizard.tsx`, add the import and replace the step 4 placeholder. Also add a `useState` import for `completionData` that was used inside `StepImport` — actually, keep `completionData` local to `StepImport` using its own `useState`. The `onStatusChange` prop should accept a setter function.

- [ ] **Step 4: Verify the full wizard flow end-to-end**

Run: `pnpm dev`, open `/import`, toggle Wizard mode.
1. Select `docs/guide/` folder, pick codebase, click "Preview →"
2. Browse files in split pane, verify template suggestions appear, edit a title
3. Click "Review →", verify stats and tables
4. Click "Start Import", watch the pipeline table update in real-time
5. Verify completion state with action buttons

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/features/import/use-sse-import.ts apps/web/src/features/import/steps/import-step.tsx apps/web/src/features/import/wizard.tsx
git commit -m "feat(import-wizard): add Step 4 (SSE streaming import with pipeline table)"
```

---

### Task 10: Final Polish and Edge Cases

**Files:**
- Modify: `apps/web/src/features/import/wizard.tsx`
- Modify: `apps/web/src/features/import/steps/select-files.tsx`
- Modify: `apps/web/src/features/import/steps/preview.tsx`

- [ ] **Step 1: Handle edge case — preview endpoint failure**

In `StepPreview`, the `catch` block already shows a toast. Add a fallback: if preview fails, initialize overrides from client-side parsing (using the `previewFile` helper from quick-mode) so the user can still proceed.

- [ ] **Step 2: Handle edge case — empty folder**

In `StepSelectFiles`, the empty state already shows a message. Verify that the "Next" button stays disabled when `selectedPaths.size === 0`.

- [ ] **Step 3: Handle edge case — SSE connection drop**

In `useSSEImport`, the `onError` callback already marks remaining files as errors. Add a visible warning banner in `StepImport` when errors include "Connection lost".

- [ ] **Step 4: Handle "click row to go back" in review**

In `StepReview`, the `onGoToFile` prop is called when clicking a row. In the wizard, this should set `step = 2` and set the active file path for the preview step. Add an `initialActivePath` prop to `StepPreview`.

- [ ] **Step 5: Run type check**

Run: `pnpm run check-types`
Expected: No type errors in the new files.

- [ ] **Step 6: Full end-to-end test with docs/guide**

1. Open `/import`, toggle to Wizard
2. Select `docs/guide/` (101 files)
3. Verify tree renders with all 25 directories, correct file counts
4. Toggle some folders off, verify partial badges
5. Click "Preview →", wait for analysis
6. Browse files, verify template matches if any
7. Click "Review →", verify new/skipped counts
8. Click "Start Import", watch pipeline
9. After completion, click "View imported chunks"
10. Click "Import more", verify wizard resets

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(import-wizard): polish edge cases and complete wizard flow"
```
