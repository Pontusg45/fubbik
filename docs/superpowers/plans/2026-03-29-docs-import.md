# Markdown Docs Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable importing folders of markdown documents as chunks via API, CLI, and web UI.

**Architecture:** API-first — a new `POST /api/chunks/import-docs` endpoint parses YAML frontmatter, extracts titles, derives tags from folder paths, and creates chunks. CLI and web UI are thin clients that read files and send `{path, content}` arrays to the endpoint.

**Tech Stack:** Elysia routes, Effect for service composition, vitest for tests, Commander.js for CLI, TanStack Router/Query + base-ui for web.

---

### Task 1: Server-Side Markdown Parser

**Files:**
- Create: `packages/api/src/chunks/parse-docs.ts`
- Create: `packages/api/src/chunks/parse-docs.test.ts`

- [ ] **Step 1: Write failing tests for the parser**

Create `packages/api/src/chunks/parse-docs.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { parseDocFile } from "./parse-docs";

describe("parseDocFile", () => {
    it("extracts title from frontmatter", () => {
        const result = parseDocFile("guides/setup.md", "---\ntitle: Setup Guide\n---\n\nSome content here.");
        expect(result.title).toBe("Setup Guide");
        expect(result.content).toBe("Some content here.");
    });

    it("extracts title from first H1 heading when no frontmatter title", () => {
        const result = parseDocFile("docs/intro.md", "# Introduction\n\nWelcome to the project.");
        expect(result.title).toBe("Introduction");
        expect(result.content).toBe("Welcome to the project.");
    });

    it("falls back to filename when no frontmatter title or heading", () => {
        const result = parseDocFile("notes/my-cool-notes.md", "Just some text without a heading.");
        expect(result.title).toBe("my cool notes");
        expect(result.content).toBe("Just some text without a heading.");
    });

    it("extracts type from frontmatter", () => {
        const result = parseDocFile("api.md", "---\ntype: reference\n---\n\n# API Docs\n\nContent.");
        expect(result.type).toBe("reference");
    });

    it("defaults type to document", () => {
        const result = parseDocFile("readme.md", "# Readme\n\nHello.");
        expect(result.type).toBe("document");
    });

    it("extracts tags from frontmatter", () => {
        const result = parseDocFile("guide.md", "---\ntags:\n  - setup\n  - onboarding\n---\n\n# Guide\n\nContent.");
        expect(result.tags).toContain("setup");
        expect(result.tags).toContain("onboarding");
    });

    it("derives tags from folder path", () => {
        const result = parseDocFile("guides/api/auth.md", "# Auth\n\nContent.");
        expect(result.tags).toContain("guides");
        expect(result.tags).toContain("api");
    });

    it("merges frontmatter tags and folder tags without duplicates", () => {
        const result = parseDocFile("guides/setup.md", "---\ntags:\n  - guides\n  - extra\n---\n\n# Setup\n\nContent.");
        const guideCount = result.tags.filter(t => t === "guides").length;
        expect(guideCount).toBe(1);
        expect(result.tags).toContain("extra");
    });

    it("handles file with no content after frontmatter", () => {
        const result = parseDocFile("empty.md", "---\ntitle: Empty\n---\n");
        expect(result.title).toBe("Empty");
        expect(result.content).toBe("");
    });

    it("handles completely empty file", () => {
        const result = parseDocFile("blank.md", "");
        expect(result.title).toBe("blank");
        expect(result.content).toBe("");
    });

    it("extracts scope from frontmatter", () => {
        const result = parseDocFile("scoped.md", "---\nscope:\n  env: production\n---\n\n# Scoped\n\nContent.");
        expect(result.scope).toEqual({ env: "production" });
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd packages/api && npx vitest run src/chunks/parse-docs.test.ts`
Expected: FAIL — `parseDocFile` is not defined.

- [ ] **Step 3: Implement the parser**

Create `packages/api/src/chunks/parse-docs.ts`:

```typescript
export interface ParsedDoc {
    title: string;
    content: string;
    type: string;
    tags: string[];
    scope?: Record<string, string>;
}

export function parseDocFile(path: string, raw: string): ParsedDoc {
    const { frontmatter, body } = extractFrontmatter(raw);

    // Title: frontmatter > first H1 > filename
    let title = frontmatter.title as string | undefined;
    let content = body;

    if (!title) {
        const headingMatch = content.match(/^#\s+(.+)$/m);
        if (headingMatch) {
            title = headingMatch[1]!.trim();
            // Remove the heading line from content
            content = content.replace(/^#\s+.+\n?/, "").trim();
        }
    }

    if (!title) {
        // Derive from filename: "my-cool-notes.md" -> "my cool notes"
        const filename = path.split("/").pop() ?? path;
        title = filename.replace(/\.md$/i, "").replace(/[-_]/g, " ");
    }

    // Tags: frontmatter + folder path segments
    const fmTags = Array.isArray(frontmatter.tags) ? frontmatter.tags.map(String) : [];
    const folderTags = tagsFromPath(path);
    const tags = [...new Set([...fmTags, ...folderTags])];

    // Type: frontmatter or default "document"
    const type = (frontmatter.type as string) ?? "document";

    // Scope
    const scope =
        frontmatter.scope && typeof frontmatter.scope === "object" && !Array.isArray(frontmatter.scope)
            ? (frontmatter.scope as Record<string, string>)
            : undefined;

    return { title, content: content.trim(), type, tags, ...(scope ? { scope } : {}) };
}

function tagsFromPath(path: string): string[] {
    const parts = path.split("/");
    parts.pop(); // remove filename
    return parts.filter(Boolean);
}

function extractFrontmatter(raw: string): { frontmatter: Record<string, unknown>; body: string } {
    const fmRegex = /^---\n([\s\S]*?)\n---\n?([\s\S]*)$/;
    const match = raw.match(fmRegex);
    if (!match) return { frontmatter: {}, body: raw };

    const yamlStr = match[1]!;
    const body = (match[2] ?? "").trim();

    // Simple YAML parser for the fields we care about
    const frontmatter: Record<string, unknown> = {};
    let currentKey: string | null = null;
    let currentArray: string[] | null = null;

    for (const line of yamlStr.split("\n")) {
        const arrayItemMatch = line.match(/^\s+-\s+(.+)$/);
        if (arrayItemMatch && currentKey) {
            if (!currentArray) currentArray = [];
            currentArray.push(arrayItemMatch[1]!.trim());
            continue;
        }

        // Flush previous array
        if (currentKey && currentArray) {
            frontmatter[currentKey] = currentArray;
            currentArray = null;
        }

        const kvMatch = line.match(/^(\w+):\s*(.*)$/);
        if (kvMatch) {
            currentKey = kvMatch[1]!;
            const value = kvMatch[2]!.trim();
            if (value) {
                frontmatter[currentKey] = value;
                currentKey = null; // not expecting array items
            }
            // If value is empty, might be followed by array items
        }

        // Nested object (scope)
        const nestedMatch = line.match(/^\s+(\w+):\s+(.+)$/);
        if (nestedMatch && currentKey && !arrayItemMatch) {
            if (typeof frontmatter[currentKey] !== "object" || Array.isArray(frontmatter[currentKey])) {
                frontmatter[currentKey] = {};
            }
            (frontmatter[currentKey] as Record<string, string>)[nestedMatch[1]!] = nestedMatch[2]!.trim();
        }
    }

    // Flush trailing array
    if (currentKey && currentArray) {
        frontmatter[currentKey] = currentArray;
    }

    return { frontmatter, body };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd packages/api && npx vitest run src/chunks/parse-docs.test.ts`
Expected: All 10 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/chunks/parse-docs.ts packages/api/src/chunks/parse-docs.test.ts
git commit -m "feat: add markdown doc parser with frontmatter and folder-tag extraction"
```

---

### Task 2: API Endpoint

**Files:**
- Modify: `packages/api/src/chunks/service.ts` — add `importDocs` function
- Modify: `packages/api/src/chunks/routes.ts` — add `/chunks/import-docs` route

- [ ] **Step 1: Add importDocs service function**

In `packages/api/src/chunks/service.ts`, add after the existing `importChunks` function (around line 302):

```typescript
import { parseDocFile } from "./parse-docs";

export function importDocs(
    userId: string,
    files: { path: string; content: string }[],
    codebaseId: string
) {
    const results: { created: number; skipped: number; errors: { path: string; error: string }[] } = {
        created: 0,
        skipped: 0,
        errors: []
    };

    return Effect.forEach(
        files,
        file =>
            Effect.try(() => parseDocFile(file.path, file.content)).pipe(
                Effect.flatMap(parsed => {
                    if (!parsed.content && !parsed.title) {
                        results.skipped++;
                        return Effect.void;
                    }
                    return createChunk(userId, {
                        title: parsed.title,
                        content: parsed.content,
                        type: parsed.type,
                        tags: parsed.tags,
                        codebaseIds: [codebaseId]
                    }).pipe(
                        Effect.tap(() => {
                            results.created++;
                            return Effect.void;
                        })
                    );
                }),
                Effect.catchAll(err => {
                    results.errors.push({ path: file.path, error: String(err) });
                    return Effect.void;
                })
            ),
        { concurrency: 10 }
    ).pipe(Effect.map(() => results));
}
```

- [ ] **Step 2: Add the route**

In `packages/api/src/chunks/routes.ts`, add after the existing `.post("/chunks/import", ...)` block (around line 63):

```typescript
.post(
    "/chunks/import-docs",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    chunkService.importDocs(session.user.id, ctx.body.files, ctx.body.codebaseId)
                )
            )
        ),
    {
        body: t.Object({
            files: t.Array(
                t.Object({
                    path: t.String({ maxLength: 500 }),
                    content: t.String({ maxLength: 100000 })
                }),
                { maxItems: 500 }
            ),
            codebaseId: t.String()
        })
    }
)
```

- [ ] **Step 3: Run type-check**

Run: `cd packages/api && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 4: Run existing tests to verify no regressions**

Run: `cd packages/api && npx vitest run`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/chunks/service.ts packages/api/src/chunks/routes.ts
git commit -m "feat: add POST /api/chunks/import-docs endpoint"
```

---

### Task 3: CLI Command

**Files:**
- Create: `apps/cli/src/commands/import-docs.ts`
- Modify: `apps/cli/src/index.ts` — register the command

- [ ] **Step 1: Create the import-docs command**

Create `apps/cli/src/commands/import-docs.ts`:

```typescript
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import { Command } from "commander";

import { resolveCodebaseId } from "../lib/detect-codebase";
import { formatSuccess, output, outputError, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

function collectMarkdownFiles(dir: string): string[] {
    const files: string[] = [];
    const entries = readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
        const fullPath = join(dir, entry.name);
        if (entry.isDirectory()) {
            files.push(...collectMarkdownFiles(fullPath));
        } else if (entry.isFile() && entry.name.endsWith(".md")) {
            files.push(fullPath);
        }
    }
    return files;
}

export const importDocsCommand = new Command("import-docs")
    .description("Import a folder of markdown documents as chunks")
    .argument("<path>", "path to directory containing .md files")
    .requiredOption("--codebase <name>", "codebase name (required)")
    .action(async (dirPath: string, opts: { codebase: string }, cmd: Command) => {
        const stat = statSync(dirPath);
        if (!stat.isDirectory()) {
            outputError(`${dirPath} is not a directory`);
            process.exit(1);
        }

        const serverUrl = getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Run 'fubbik init' first.");
            process.exit(1);
        }

        const codebaseId = await resolveCodebaseId(serverUrl, { codebase: opts.codebase });
        if (!codebaseId) {
            outputError(`Could not resolve codebase "${opts.codebase}".`);
            process.exit(1);
        }

        const mdFiles = collectMarkdownFiles(dirPath);
        if (mdFiles.length === 0) {
            outputError("No .md files found in the directory.");
            process.exit(1);
        }

        if (mdFiles.length > 500) {
            outputError(`Found ${mdFiles.length} files, max is 500. Import in smaller batches.`);
            process.exit(1);
        }

        const files = mdFiles.map(fullPath => ({
            path: relative(dirPath, fullPath),
            content: readFileSync(fullPath, "utf-8")
        }));

        try {
            const res = await fetch(`${serverUrl}/api/chunks/import-docs`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ files, codebaseId })
            });

            if (!res.ok) {
                const text = await res.text();
                outputError(`Server error (${res.status}): ${text}`);
                process.exit(1);
            }

            const data = (await res.json()) as {
                created: number;
                skipped: number;
                errors: { path: string; error: string }[];
            };

            if (data.errors.length > 0) {
                for (const err of data.errors) {
                    console.error(`  Error: ${err.path} — ${err.error}`);
                }
            }

            outputQuiet(cmd, String(data.created));
            output(
                cmd,
                data,
                formatSuccess(
                    `Created: ${data.created} | Skipped: ${data.skipped} | Errors: ${data.errors.length}`
                )
            );
        } catch (err) {
            outputError(`Failed to connect to server: ${err}`);
            process.exit(1);
        }
    });
```

- [ ] **Step 2: Register the command in CLI index**

In `apps/cli/src/index.ts`, add the import at the top with the other imports:

```typescript
import { importDocsCommand } from "./commands/import-docs";
```

Add below the existing `program.addCommand(importDirCommand);` line (around line 69):

```typescript
program.addCommand(importDocsCommand);
```

- [ ] **Step 3: Verify CLI builds**

Run: `cd apps/cli && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 4: Commit**

```bash
git add apps/cli/src/commands/import-docs.ts apps/cli/src/index.ts
git commit -m "feat: add fubbik import-docs CLI command"
```

---

### Task 4: Web UI — Import Dialog on Chunks List

**Files:**
- Create: `apps/web/src/features/import/import-dialog.tsx`
- Modify: `apps/web/src/routes/chunks.index.tsx` — add Import Docs button

- [ ] **Step 1: Create the import dialog component**

Create `apps/web/src/features/import/import-dialog.tsx`:

```typescript
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderUp, Upload } from "lucide-react";
import { useRef, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Dialog, DialogClose, DialogPopup, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface FileEntry {
    path: string;
    content: string;
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

export function ImportDocsDialog({ trigger }: { trigger: React.ReactNode }) {
    const queryClient = useQueryClient();
    const folderInputRef = useRef<HTMLInputElement>(null);
    const [files, setFiles] = useState<FileEntry[]>([]);
    const [codebaseId, setCodebaseId] = useState<string>("");

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
        onSuccess: data => {
            const msg = `Created: ${data.created} | Skipped: ${data.skipped} | Errors: ${data.errors.length}`;
            if (data.errors.length > 0) {
                toast.warning(msg);
            } else {
                toast.success(msg);
            }
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            setFiles([]);
            setCodebaseId("");
        },
        onError: () => toast.error("Failed to import docs")
    });

    const handleFolderSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
        if (!e.target.files) return;
        const entries = await readFilesFromInput(e.target.files);
        setFiles(entries);
        e.target.value = "";
    };

    const handleImport = () => {
        if (!codebaseId) {
            toast.error("Please select a codebase");
            return;
        }
        if (files.length === 0) {
            toast.error("No markdown files selected");
            return;
        }
        if (files.length > 500) {
            toast.error("Too many files (max 500). Import in smaller batches.");
            return;
        }
        importMutation.mutate({ files, codebaseId });
    };

    return (
        <Dialog
            onOpenChange={open => {
                if (!open) {
                    setFiles([]);
                    setCodebaseId("");
                }
            }}
        >
            <DialogTrigger render={trigger} />
            <DialogPopup>
                <DialogHeader>
                    <DialogTitle>Import Markdown Docs</DialogTitle>
                </DialogHeader>

                <div className="space-y-4 py-4">
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
                        <div className="mt-1 flex items-center gap-2">
                            <Button
                                variant="outline"
                                size="sm"
                                onClick={() => folderInputRef.current?.click()}
                            >
                                <FolderUp className="mr-1 size-3.5" />
                                Select Folder
                            </Button>
                            {files.length > 0 && (
                                <span className="text-muted-foreground text-sm">
                                    {files.length} markdown file{files.length !== 1 ? "s" : ""} found
                                </span>
                            )}
                        </div>
                    </div>

                    <div>
                        <label className="text-sm font-medium">Codebase</label>
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
                </div>

                <div className="flex justify-end gap-2">
                    <DialogClose render={<Button variant="outline" size="sm" />}>Cancel</DialogClose>
                    <Button
                        size="sm"
                        onClick={handleImport}
                        disabled={importMutation.isPending || files.length === 0 || !codebaseId}
                    >
                        <Upload className="mr-1 size-3.5" />
                        {importMutation.isPending ? "Importing..." : `Import ${files.length} file${files.length !== 1 ? "s" : ""}`}
                    </Button>
                </div>
            </DialogPopup>
        </Dialog>
    );
}
```

- [ ] **Step 2: Add Import Docs button to chunks list page**

In `apps/web/src/routes/chunks.index.tsx`, add the import at the top:

```typescript
import { ImportDocsDialog } from "@/features/import/import-dialog";
```

Add the `FolderUp` icon to the existing lucide-react import.

Find the header actions area (around line 526-535, the `<div className="flex items-center gap-2">` containing "View archived" and "New Chunk"). Add the import button before the "New Chunk" button:

```typescript
<ImportDocsDialog
    trigger={
        <Button variant="outline" size="sm">
            <FolderUp className="mr-1 size-3.5" />
            Import Docs
        </Button>
    }
/>
```

- [ ] **Step 3: Verify web app builds**

Run: `cd apps/web && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/features/import/import-dialog.tsx apps/web/src/routes/chunks.index.tsx
git commit -m "feat: add Import Docs dialog to chunks list page"
```

---

### Task 5: Web UI — Dedicated Import Page

**Files:**
- Create: `apps/web/src/routes/import.tsx`

- [ ] **Step 1: Create the import page**

Create `apps/web/src/routes/import.tsx`:

```typescript
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Check, FileText, FolderUp, Trash2, Upload, X } from "lucide-react";
import { useCallback, useRef, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { PageContainer, PageHeader } from "@/components/ui/page";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface FileEntry {
    path: string;
    content: string;
}

interface PreviewRow {
    path: string;
    content: string;
    title: string;
    tags: string[];
    type: string;
    selected: boolean;
}

function previewFile(path: string, content: string): Omit<PreviewRow, "selected"> {
    // Client-side preview extraction (mirrors server logic for display only)
    let title = "";
    let type = "document";
    const tags: string[] = [];

    // Extract frontmatter
    const fmMatch = content.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
    const body = fmMatch ? (fmMatch[2] ?? "").trim() : content;
    if (fmMatch) {
        const yaml = fmMatch[1] ?? "";
        const titleMatch = yaml.match(/^title:\s+(.+)$/m);
        if (titleMatch) title = titleMatch[1]!.trim();
        const typeMatch = yaml.match(/^type:\s+(.+)$/m);
        if (typeMatch) type = typeMatch[1]!.trim();
    }

    if (!title) {
        const headingMatch = body.match(/^#\s+(.+)$/m);
        if (headingMatch) title = headingMatch[1]!.trim();
    }

    if (!title) {
        const filename = path.split("/").pop() ?? path;
        title = filename.replace(/\.md$/i, "").replace(/[-_]/g, " ");
    }

    // Folder tags
    const parts = path.split("/");
    parts.pop();
    tags.push(...parts.filter(Boolean));

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

export const Route = createFileRoute("/import")({
    component: ImportPage,
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

function ImportPage() {
    const queryClient = useQueryClient();
    const folderInputRef = useRef<HTMLInputElement>(null);
    const [rows, setRows] = useState<PreviewRow[]>([]);
    const [codebaseId, setCodebaseId] = useState<string>("");
    const [result, setResult] = useState<{
        created: number;
        skipped: number;
        errors: { path: string; error: string }[];
    } | null>(null);

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
        onSuccess: data => {
            setResult(data);
            if (data.errors.length > 0) {
                toast.warning(`Imported with ${data.errors.length} error(s)`);
            } else {
                toast.success(`Created ${data.created} chunks`);
            }
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
        },
        onError: () => toast.error("Failed to import docs")
    });

    const handleFolderSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
        if (!e.target.files) return;
        const entries = await readFilesFromInput(e.target.files);
        const previews = entries.map(f => ({ ...previewFile(f.path, f.content), selected: true }));
        setRows(previews);
        setResult(null);
        e.target.value = "";
    };

    const toggleRow = useCallback((index: number) => {
        setRows(prev => prev.map((r, i) => (i === index ? { ...r, selected: !r.selected } : r)));
    }, []);

    const toggleAll = useCallback(() => {
        setRows(prev => {
            const allSelected = prev.every(r => r.selected);
            return prev.map(r => ({ ...r, selected: !allSelected }));
        });
    }, []);

    const selectedRows = rows.filter(r => r.selected);

    const handleImport = () => {
        if (!codebaseId) {
            toast.error("Please select a codebase");
            return;
        }
        const files = selectedRows.map(r => ({ path: r.path, content: r.content }));
        if (files.length === 0) {
            toast.error("No files selected");
            return;
        }
        if (files.length > 500) {
            toast.error("Too many files (max 500). Deselect some or import in batches.");
            return;
        }
        importMutation.mutate({ files, codebaseId });
    };

    return (
        <PageContainer>
            <PageHeader icon={Upload} title="Import Docs" description="Import a folder of markdown documents as chunks" />

            {/* Controls */}
            <div className="mb-6 flex flex-wrap items-center gap-3">
                <input
                    ref={folderInputRef}
                    type="file"
                    // @ts-expect-error webkitdirectory is non-standard
                    webkitdirectory=""
                    className="hidden"
                    onChange={handleFolderSelect}
                />
                <Button variant="outline" onClick={() => folderInputRef.current?.click()}>
                    <FolderUp className="mr-1.5 size-4" />
                    Select Folder
                </Button>

                <select
                    className="border-input bg-background rounded-md border px-3 py-2 text-sm"
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

                {rows.length > 0 && (
                    <>
                        <span className="text-muted-foreground text-sm">
                            {selectedRows.length} of {rows.length} files selected
                        </span>
                        <Button
                            onClick={handleImport}
                            disabled={importMutation.isPending || selectedRows.length === 0 || !codebaseId}
                        >
                            <Upload className="mr-1.5 size-4" />
                            {importMutation.isPending
                                ? "Importing..."
                                : `Import ${selectedRows.length} file${selectedRows.length !== 1 ? "s" : ""}`}
                        </Button>
                    </>
                )}
            </div>

            {/* Result summary */}
            {result && (
                <Card className="mb-6">
                    <CardPanel className="flex items-center gap-4">
                        <div className="flex items-center gap-1.5">
                            <Check className="size-4 text-green-500" />
                            <span className="text-sm font-medium">{result.created} created</span>
                        </div>
                        {result.skipped > 0 && (
                            <div className="flex items-center gap-1.5">
                                <X className="text-muted-foreground size-4" />
                                <span className="text-muted-foreground text-sm">{result.skipped} skipped</span>
                            </div>
                        )}
                        {result.errors.length > 0 && (
                            <div className="flex items-center gap-1.5">
                                <X className="size-4 text-red-500" />
                                <span className="text-sm text-red-500">{result.errors.length} errors</span>
                            </div>
                        )}
                        <Link to="/chunks" search={{}} className="text-sm underline">
                            View chunks
                        </Link>
                    </CardPanel>
                    {result.errors.length > 0 && (
                        <CardPanel className="border-t">
                            <ul className="space-y-1 text-sm">
                                {result.errors.map((err, i) => (
                                    <li key={i} className="text-red-500">
                                        <span className="font-mono">{err.path}</span> — {err.error}
                                    </li>
                                ))}
                            </ul>
                        </CardPanel>
                    )}
                </Card>
            )}

            {/* Preview table */}
            {rows.length > 0 && !result && (
                <div className="overflow-x-auto rounded-md border">
                    <table className="w-full text-sm">
                        <thead className="bg-muted/50 border-b">
                            <tr>
                                <th className="w-10 px-3 py-2">
                                    <Checkbox
                                        checked={rows.every(r => r.selected)}
                                        indeterminate={rows.some(r => r.selected) && !rows.every(r => r.selected)}
                                        onCheckedChange={toggleAll}
                                    />
                                </th>
                                <th className="px-3 py-2 text-left font-medium">Path</th>
                                <th className="px-3 py-2 text-left font-medium">Title</th>
                                <th className="px-3 py-2 text-left font-medium">Tags</th>
                                <th className="px-3 py-2 text-left font-medium">Type</th>
                            </tr>
                        </thead>
                        <tbody>
                            {rows.map((row, i) => (
                                <tr
                                    key={row.path}
                                    className={`border-b last:border-0 ${!row.selected ? "opacity-40" : ""}`}
                                >
                                    <td className="px-3 py-2">
                                        <Checkbox checked={row.selected} onCheckedChange={() => toggleRow(i)} />
                                    </td>
                                    <td className="px-3 py-2 font-mono text-xs">{row.path}</td>
                                    <td className="px-3 py-2">{row.title}</td>
                                    <td className="px-3 py-2">
                                        <div className="flex flex-wrap gap-1">
                                            {row.tags.map(tag => (
                                                <Badge key={tag} variant="secondary" size="sm">
                                                    {tag}
                                                </Badge>
                                            ))}
                                        </div>
                                    </td>
                                    <td className="px-3 py-2">
                                        <Badge variant="outline" size="sm">
                                            {row.type}
                                        </Badge>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            )}

            {/* Empty state */}
            {rows.length === 0 && !result && (
                <div className="flex flex-col items-center justify-center rounded-lg border-2 border-dashed py-16">
                    <FileText className="text-muted-foreground mb-3 size-10" />
                    <p className="text-muted-foreground mb-1 text-sm font-medium">No files selected</p>
                    <p className="text-muted-foreground mb-4 text-xs">
                        Select a folder containing markdown files to preview and import
                    </p>
                    <Button variant="outline" onClick={() => folderInputRef.current?.click()}>
                        <FolderUp className="mr-1.5 size-4" />
                        Select Folder
                    </Button>
                </div>
            )}
        </PageContainer>
    );
}
```

- [ ] **Step 2: Verify web app builds**

Run: `cd apps/web && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/import.tsx
git commit -m "feat: add dedicated /import page with preview table"
```

---

### Task 6: Navigation Link

**Files:**
- Modify: `apps/web/src/routes/__root.tsx` — add /import nav link

- [ ] **Step 1: Add nav link**

In `apps/web/src/routes/__root.tsx`, find the nav links section and add `/import` link alongside the other navigation items. Add it after an appropriate existing link (e.g., after "Chunks"):

```typescript
<Link
    to="/import"
    className="text-muted-foreground hover:text-foreground [&.active]:text-foreground rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
>
    Import
</Link>
```

Also add the same link to the mobile nav in `apps/web/src/features/nav/mobile-nav.tsx` if it follows the same pattern.

- [ ] **Step 2: Verify web app builds**

Run: `cd apps/web && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/__root.tsx apps/web/src/features/nav/mobile-nav.tsx
git commit -m "feat: add Import nav link"
```

---

### Task 7: End-to-End Verification

- [ ] **Step 1: Run full test suite**

Run: `pnpm test`
Expected: All tests pass.

- [ ] **Step 2: Run type-check across all packages**

Run: `pnpm run check-types`
Expected: No type errors.

- [ ] **Step 3: Manual verification checklist**

If the dev server is running (`pnpm dev`):
- Create a test folder with 3-4 `.md` files (some with frontmatter, some without, in nested subdirectories)
- Test CLI: `fubbik import-docs ./test-docs --codebase <name>`
- Test web dialog: Go to `/chunks`, click "Import Docs", select the folder, pick codebase, import
- Test web page: Go to `/import`, select the folder, verify preview table, import
- Verify chunks appear in the chunks list with correct titles, tags, types

- [ ] **Step 4: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "fix: address import-docs verification issues"
```
