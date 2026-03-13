# VS Code / Cursor Extension Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a VS Code / Cursor extension with a sidebar showing codebase chunks and a command to create chunks from selected text.

**Architecture:** Standalone `apps/vscode/` package communicating with the fubbik API via HTTP. Webview sidebar for the chunk list, webview panel for the create form. Codebase detection via git remote (async exec) with local path fallback. Bundled to CJS via esbuild.

**Tech Stack:** VS Code Extension API, TypeScript, esbuild, vanilla HTML/CSS/JS for webviews

**Spec:** `docs/superpowers/specs/2026-03-13-vscode-extension-design.md`

---

## File Structure

### New files (all under `apps/vscode/`)
- `package.json` — extension manifest + npm scripts
- `tsconfig.json` — TypeScript config targeting ES2022, CJS output
- `.vscodeignore` — excludes src/, node_modules/ from .vsix
- `esbuild.mjs` — build script bundling to dist/extension.js
- `.vscode/launch.json` — F5 debug profile
- `.vscode/tasks.json` — build task
- `src/extension.ts` — activate/deactivate entry point
- `src/api.ts` — FubbikApi class (fetch-based HTTP client)
- `src/detect-codebase.ts` — async git remote detection + fallback
- `src/sidebar-provider.ts` — WebviewViewProvider for chunk list
- `src/create-chunk.ts` — "Add to Fubbik" command + webview panel
- `src/webview-utils.ts` — shared nonce generation + HTML helpers

---

## Chunk 1: Project Scaffolding

### Task 1: Initialize the extension package

**Files:**
- Create: `apps/vscode/package.json`
- Create: `apps/vscode/tsconfig.json`
- Create: `apps/vscode/.vscodeignore`
- Create: `apps/vscode/.vscode/launch.json`
- Create: `apps/vscode/.vscode/tasks.json`
- Create: `apps/vscode/esbuild.mjs`

- [ ] **Step 1: Create `apps/vscode/package.json`**

```json
{
  "name": "fubbik-vscode",
  "displayName": "Fubbik",
  "description": "Fubbik knowledge base integration for VS Code and Cursor",
  "version": "0.0.1",
  "publisher": "fubbik",
  "engines": {
    "vscode": "^1.85.0"
  },
  "categories": ["Other"],
  "main": "./dist/extension.js",
  "activationEvents": ["onStartupFinished"],
  "contributes": {
    "viewsContainers": {
      "activitybar": [
        {
          "id": "fubbik",
          "title": "Fubbik",
          "icon": "resources/icon.svg"
        }
      ]
    },
    "views": {
      "fubbik": [
        {
          "type": "webview",
          "id": "fubbik.sidebar",
          "name": "Chunks"
        }
      ]
    },
    "commands": [
      {
        "command": "fubbik.addChunk",
        "title": "Fubbik: Add to Knowledge Base"
      },
      {
        "command": "fubbik.refreshSidebar",
        "title": "Fubbik: Refresh Chunks"
      }
    ],
    "menus": {
      "editor/context": [
        {
          "command": "fubbik.addChunk",
          "when": "editorHasSelection",
          "group": "fubbik"
        }
      ]
    },
    "configuration": {
      "title": "Fubbik",
      "properties": {
        "fubbik.serverUrl": {
          "type": "string",
          "default": "http://localhost:3000",
          "description": "Fubbik API server URL"
        },
        "fubbik.webAppUrl": {
          "type": "string",
          "default": "http://localhost:3001",
          "description": "Fubbik web app URL (for opening chunks in browser)"
        }
      }
    }
  },
  "scripts": {
    "build": "node esbuild.mjs",
    "watch": "node esbuild.mjs --watch",
    "package": "vsce package"
  },
  "devDependencies": {
    "@types/vscode": "^1.85.0",
    "esbuild": "^0.24.0",
    "typescript": "^5.7.0"
  }
}
```

- [ ] **Step 2: Create `apps/vscode/tsconfig.json`**

```json
{
  "compilerOptions": {
    "module": "commonjs",
    "target": "ES2022",
    "outDir": "dist",
    "lib": ["ES2022"],
    "sourceMap": true,
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "rootDir": "src"
  },
  "include": ["src"],
  "exclude": ["node_modules", "dist"]
}
```

- [ ] **Step 3: Create `apps/vscode/.vscodeignore`**

```
src/
node_modules/
.vscode/
tsconfig.json
esbuild.mjs
*.map
```

- [ ] **Step 4: Create `apps/vscode/esbuild.mjs`**

```javascript
import * as esbuild from "esbuild";

const watch = process.argv.includes("--watch");

const ctx = await esbuild.context({
    entryPoints: ["src/extension.ts"],
    bundle: true,
    outfile: "dist/extension.js",
    external: ["vscode"],
    format: "cjs",
    platform: "node",
    target: "es2022",
    sourcemap: true,
    minify: !watch
});

if (watch) {
    await ctx.watch();
    console.log("Watching for changes...");
} else {
    await ctx.rebuild();
    await ctx.dispose();
    console.log("Build complete.");
}
```

- [ ] **Step 5: Create `apps/vscode/.vscode/launch.json`**

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Run Extension",
      "type": "extensionHost",
      "request": "launch",
      "args": ["--extensionDevelopmentPath=${workspaceFolder}/apps/vscode"],
      "outFiles": ["${workspaceFolder}/apps/vscode/dist/**/*.js"],
      "preLaunchTask": "build-vscode-ext"
    }
  ]
}
```

- [ ] **Step 6: Create `apps/vscode/.vscode/tasks.json`**

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "build-vscode-ext",
      "type": "shell",
      "command": "cd apps/vscode && node esbuild.mjs",
      "problemMatcher": ["$tsc"]
    }
  ]
}
```

- [ ] **Step 7: Create a simple icon**

Create `apps/vscode/resources/icon.svg` — a minimal SVG placeholder:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="12" cy="5" r="3"/>
  <circle cx="19" cy="17" r="3"/>
  <circle cx="5" cy="17" r="3"/>
  <line x1="12" y1="8" x2="19" y2="14"/>
  <line x1="12" y1="8" x2="5" y2="14"/>
</svg>
```

- [ ] **Step 8: Install dependencies**

Run: `cd apps/vscode && pnpm install`

- [ ] **Step 9: Commit**

```bash
git add apps/vscode/package.json apps/vscode/tsconfig.json apps/vscode/.vscodeignore apps/vscode/esbuild.mjs apps/vscode/.vscode apps/vscode/resources
git commit -m "feat(vscode): scaffold VS Code extension package"
```

---

## Chunk 2: Core Modules

### Task 2: Webview utilities

**Files:**
- Create: `apps/vscode/src/webview-utils.ts`

- [ ] **Step 1: Write the utility module**

```typescript
// apps/vscode/src/webview-utils.ts
import * as vscode from "vscode";

export function getNonce(): string {
    let text = "";
    const possible = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    for (let i = 0; i < 32; i++) {
        text += possible.charAt(Math.floor(Math.random() * possible.length));
    }
    return text;
}

export function getBaseHtml(webview: vscode.Webview, nonce: string, body: string, script: string): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'nonce-${nonce}'; script-src 'nonce-${nonce}';">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style nonce="${nonce}">
        body {
            font-family: var(--vscode-font-family);
            font-size: var(--vscode-font-size);
            color: var(--vscode-foreground);
            background: var(--vscode-sideBar-background, var(--vscode-editor-background));
            padding: 0;
            margin: 0;
        }
        .container { padding: 12px; }
        input, textarea, select {
            background: var(--vscode-input-background);
            color: var(--vscode-input-foreground);
            border: 1px solid var(--vscode-input-border);
            border-radius: 2px;
            padding: 4px 8px;
            font-family: inherit;
            font-size: inherit;
            width: 100%;
            box-sizing: border-box;
        }
        textarea { resize: vertical; min-height: 100px; }
        button {
            background: var(--vscode-button-background);
            color: var(--vscode-button-foreground);
            border: none;
            border-radius: 2px;
            padding: 6px 14px;
            cursor: pointer;
            font-size: inherit;
        }
        button:hover { background: var(--vscode-button-hoverBackground); }
        button.secondary {
            background: var(--vscode-button-secondaryBackground);
            color: var(--vscode-button-secondaryForeground);
        }
        .badge {
            display: inline-block;
            background: var(--vscode-badge-background);
            color: var(--vscode-badge-foreground);
            border-radius: 10px;
            padding: 1px 6px;
            font-size: 10px;
            font-weight: 500;
        }
        a {
            color: var(--vscode-textLink-foreground);
            text-decoration: none;
            cursor: pointer;
        }
        a:hover { text-decoration: underline; }
    </style>
</head>
<body>
    <div class="container">${body}</div>
    <script nonce="${nonce}">${script}</script>
</body>
</html>`;
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/vscode/src/webview-utils.ts
git commit -m "feat(vscode): add webview utility helpers (nonce, base HTML)"
```

---

### Task 3: API client

**Files:**
- Create: `apps/vscode/src/api.ts`

- [ ] **Step 1: Write the API client**

```typescript
// apps/vscode/src/api.ts
export interface Chunk {
    id: string;
    title: string;
    content: string;
    type: string;
    createdAt: string;
    updatedAt: string;
}

export interface CreateChunkBody {
    title: string;
    content: string;
    type: string;
    tags?: string[];
    codebaseIds?: string[];
}

export interface DetectResult {
    id: string;
    name: string;
}

export class FubbikApi {
    constructor(private serverUrl: string) {}

    async detectCodebase(params: { remoteUrl?: string; localPath?: string }): Promise<DetectResult | null> {
        const query = new URLSearchParams();
        if (params.remoteUrl) query.set("remoteUrl", params.remoteUrl);
        else if (params.localPath) query.set("localPath", params.localPath);
        else return null;

        try {
            const res = await fetch(`${this.serverUrl}/api/codebases/detect?${query}`);
            if (!res.ok) return null;
            const data = await res.json();
            return data && data.id ? { id: data.id, name: data.name } : null;
        } catch {
            return null;
        }
    }

    async getChunks(codebaseId?: string): Promise<{ chunks: Chunk[]; total: number }> {
        const query = new URLSearchParams({ limit: "100" });
        if (codebaseId) query.set("codebaseId", codebaseId);

        const res = await fetch(`${this.serverUrl}/api/chunks?${query}`);
        if (!res.ok) throw new Error(`Failed to fetch chunks: ${res.status}`);
        const data = await res.json();
        return { chunks: data.chunks ?? [], total: data.total ?? 0 };
    }

    async createChunk(body: CreateChunkBody): Promise<Chunk> {
        const res = await fetch(`${this.serverUrl}/api/chunks`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body)
        });
        if (!res.ok) {
            const err = await res.json().catch(() => null);
            throw new Error(err?.message ?? `Failed to create chunk: ${res.status}`);
        }
        return res.json();
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/vscode/src/api.ts
git commit -m "feat(vscode): add FubbikApi HTTP client"
```

---

### Task 4: Codebase detection

**Files:**
- Create: `apps/vscode/src/detect-codebase.ts`

- [ ] **Step 1: Write the detection module**

```typescript
// apps/vscode/src/detect-codebase.ts
import { exec } from "node:child_process";
import { promisify } from "node:util";

import * as vscode from "vscode";

import { FubbikApi, type DetectResult } from "./api";

const execAsync = promisify(exec);

async function getGitRemoteUrl(cwd: string): Promise<string | null> {
    try {
        const { stdout } = await execAsync("git remote get-url origin", { cwd });
        return stdout.trim() || null;
    } catch {
        return null;
    }
}

export async function detectCodebase(api: FubbikApi): Promise<DetectResult | null> {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) return null;

    const workspacePath = folders[0]!.uri.fsPath;

    // Try git remote first
    const remoteUrl = await getGitRemoteUrl(workspacePath);
    if (remoteUrl) {
        const result = await api.detectCodebase({ remoteUrl });
        if (result) return result;
    }

    // Fall back to local path
    return api.detectCodebase({ localPath: workspacePath });
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/vscode/src/detect-codebase.ts
git commit -m "feat(vscode): add async codebase detection with local path fallback"
```

---

## Chunk 3: Sidebar

### Task 5: Sidebar webview provider

**Files:**
- Create: `apps/vscode/src/sidebar-provider.ts`

- [ ] **Step 1: Write the sidebar provider**

```typescript
// apps/vscode/src/sidebar-provider.ts
import * as vscode from "vscode";

import type { Chunk } from "./api";
import { getBaseHtml, getNonce } from "./webview-utils";

export class SidebarProvider implements vscode.WebviewViewProvider {
    public static readonly viewType = "fubbik.sidebar";
    private _view?: vscode.WebviewView;
    private _codebaseName: string | null = null;
    private _chunks: Chunk[] = [];
    private _total = 0;
    private _error: string | null = null;
    private _loading = true;

    constructor(private readonly _extensionUri: vscode.Uri) {}

    setState(opts: {
        codebaseName?: string | null;
        chunks?: Chunk[];
        total?: number;
        error?: string | null;
        loading?: boolean;
    }) {
        if (opts.codebaseName !== undefined) this._codebaseName = opts.codebaseName;
        if (opts.chunks !== undefined) this._chunks = opts.chunks;
        if (opts.total !== undefined) this._total = opts.total;
        if (opts.error !== undefined) this._error = opts.error;
        if (opts.loading !== undefined) this._loading = opts.loading;
        this._update();
    }

    resolveWebviewView(webviewView: vscode.WebviewView) {
        this._view = webviewView;
        webviewView.webview.options = { enableScripts: true };
        webviewView.webview.onDidReceiveMessage(msg => {
            if (msg.type === "openChunk") {
                const webAppUrl = vscode.workspace.getConfiguration("fubbik").get<string>("webAppUrl", "http://localhost:3001");
                vscode.env.openExternal(vscode.Uri.parse(`${webAppUrl}/chunks/${msg.id}`));
            } else if (msg.type === "refresh") {
                vscode.commands.executeCommand("fubbik.refreshSidebar");
            }
        });
        this._update();
    }

    private _update() {
        if (!this._view) return;
        const nonce = getNonce();

        let body = "";

        // Header
        body += `<div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:8px;">`;
        body += `<strong>${this._codebaseName ?? "No codebase detected"}</strong>`;
        body += `<a onclick="refresh()" style="font-size:12px;cursor:pointer;">↻</a>`;
        body += `</div>`;

        if (this._loading) {
            body += `<p style="opacity:0.6;font-size:12px;">Loading chunks...</p>`;
        } else if (this._error) {
            body += `<p style="color:var(--vscode-errorForeground);font-size:12px;">${this._error}</p>`;
        } else {
            // Search
            body += `<input type="text" id="search" placeholder="Filter chunks..." style="margin-bottom:8px;" />`;

            if (this._total > 100) {
                body += `<p style="opacity:0.5;font-size:10px;margin-bottom:6px;">Showing first 100 of ${this._total} chunks</p>`;
            }

            // Chunk list
            body += `<div id="chunkList">`;
            for (const chunk of this._chunks) {
                const date = new Date(chunk.createdAt).toLocaleDateString();
                body += `<div class="chunk-item" data-title="${chunk.title.toLowerCase()}" style="padding:6px 0;border-bottom:1px solid var(--vscode-widget-border,#333);cursor:pointer;" onclick="openChunk('${chunk.id}')">`;
                body += `<div style="display:flex;align-items:center;gap:6px;">`;
                body += `<span class="badge">${chunk.type}</span>`;
                body += `<span style="font-size:12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${chunk.title}</span>`;
                body += `</div>`;
                body += `<div style="font-size:10px;opacity:0.5;margin-top:2px;">${date}</div>`;
                body += `</div>`;
            }
            if (this._chunks.length === 0) {
                body += `<p style="opacity:0.6;font-size:12px;">No chunks found</p>`;
            }
            body += `</div>`;
        }

        const script = `
            const vscode = acquireVsCodeApi();
            function openChunk(id) { vscode.postMessage({ type: "openChunk", id }); }
            function refresh() { vscode.postMessage({ type: "refresh" }); }
            const search = document.getElementById("search");
            if (search) {
                search.addEventListener("input", () => {
                    const q = search.value.toLowerCase();
                    document.querySelectorAll(".chunk-item").forEach(el => {
                        el.style.display = el.dataset.title.includes(q) ? "" : "none";
                    });
                });
            }
        `;

        this._view.webview.html = getBaseHtml(this._view.webview, nonce, body, script);
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/vscode/src/sidebar-provider.ts
git commit -m "feat(vscode): add sidebar webview provider with chunk list"
```

---

## Chunk 4: Create Chunk Command

### Task 6: "Add to Fubbik" command

**Files:**
- Create: `apps/vscode/src/create-chunk.ts`

- [ ] **Step 1: Write the create chunk command**

```typescript
// apps/vscode/src/create-chunk.ts
import * as vscode from "vscode";

import type { FubbikApi } from "./api";
import { getBaseHtml, getNonce } from "./webview-utils";

export function registerCreateChunkCommand(
    context: vscode.ExtensionContext,
    api: FubbikApi,
    getCodebaseId: () => string | null,
    onChunkCreated: () => void
): vscode.Disposable {
    return vscode.commands.registerCommand("fubbik.addChunk", () => {
        const editor = vscode.window.activeTextEditor;
        const selection = editor?.document.getText(editor.selection) ?? "";
        const firstLine = selection.split("\n")[0]?.trim() ?? "";

        const panel = vscode.window.createWebviewPanel(
            "fubbik.createChunk",
            "Add to Fubbik",
            vscode.ViewColumn.One,
            { enableScripts: true }
        );

        const codebaseId = getCodebaseId();
        const nonce = getNonce();

        const body = `
            <h3 style="margin-top:0;">Add to Fubbik</h3>
            ${!codebaseId ? '<p style="font-size:11px;opacity:0.6;">No codebase detected — chunk will be global.</p>' : ""}
            <div style="display:flex;flex-direction:column;gap:10px;">
                <div>
                    <label style="font-size:11px;opacity:0.7;display:block;margin-bottom:2px;">Title</label>
                    <input type="text" id="title" value="${firstLine.replace(/"/g, "&quot;")}" />
                </div>
                <div>
                    <label style="font-size:11px;opacity:0.7;display:block;margin-bottom:2px;">Content</label>
                    <textarea id="content" rows="10">${selection.replace(/</g, "&lt;")}</textarea>
                </div>
                <div>
                    <label style="font-size:11px;opacity:0.7;display:block;margin-bottom:2px;">Type</label>
                    <select id="type">
                        <option value="note" selected>note</option>
                        <option value="document">document</option>
                        <option value="reference">reference</option>
                        <option value="schema">schema</option>
                        <option value="checklist">checklist</option>
                    </select>
                </div>
                <div>
                    <label style="font-size:11px;opacity:0.7;display:block;margin-bottom:2px;">Tags (comma-separated)</label>
                    <input type="text" id="tags" placeholder="e.g. auth, backend" />
                </div>
                <div style="display:flex;gap:8px;margin-top:4px;">
                    <button onclick="submit()">Create Chunk</button>
                    <button class="secondary" onclick="cancel()">Cancel</button>
                </div>
            </div>
        `;

        const script = `
            const vscode = acquireVsCodeApi();
            function submit() {
                const title = document.getElementById("title").value;
                const content = document.getElementById("content").value;
                const type = document.getElementById("type").value;
                const tagsRaw = document.getElementById("tags").value;
                const tags = tagsRaw ? tagsRaw.split(",").map(t => t.trim()).filter(Boolean) : [];
                vscode.postMessage({ type: "submit", data: { title, content, type, tags } });
            }
            function cancel() {
                vscode.postMessage({ type: "cancel" });
            }
        `;

        panel.webview.html = getBaseHtml(panel.webview, nonce, body, script);

        panel.webview.onDidReceiveMessage(async (msg: { type: string; data?: { title: string; content: string; type: string; tags: string[] } }) => {
            if (msg.type === "cancel") {
                panel.dispose();
                return;
            }
            if (msg.type === "submit" && msg.data) {
                try {
                    const chunk = await api.createChunk({
                        ...msg.data,
                        codebaseIds: codebaseId ? [codebaseId] : undefined
                    });
                    vscode.window.showInformationMessage(`Chunk created: ${chunk.title}`);
                    panel.dispose();
                    onChunkCreated();
                } catch (err) {
                    vscode.window.showErrorMessage(`Failed to create chunk: ${err instanceof Error ? err.message : String(err)}`);
                }
            }
        });
    });
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/vscode/src/create-chunk.ts
git commit -m "feat(vscode): add 'Add to Fubbik' command with webview form"
```

---

## Chunk 5: Extension Entry Point + Build

### Task 7: Extension entry point

**Files:**
- Create: `apps/vscode/src/extension.ts`

- [ ] **Step 1: Write the extension entry point**

```typescript
// apps/vscode/src/extension.ts
import * as vscode from "vscode";

import { FubbikApi } from "./api";
import { registerCreateChunkCommand } from "./create-chunk";
import { detectCodebase } from "./detect-codebase";
import { SidebarProvider } from "./sidebar-provider";

let codebaseId: string | null = null;

export async function activate(context: vscode.ExtensionContext) {
    const config = vscode.workspace.getConfiguration("fubbik");
    const serverUrl = config.get<string>("serverUrl", "http://localhost:3000");
    const api = new FubbikApi(serverUrl);
    const sidebarProvider = new SidebarProvider(context.extensionUri);

    // Register sidebar
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(SidebarProvider.viewType, sidebarProvider)
    );

    // Refresh command
    async function refreshChunks() {
        sidebarProvider.setState({ loading: true, error: null });
        try {
            const { chunks, total } = await api.getChunks(codebaseId ?? undefined);
            sidebarProvider.setState({ chunks, total, loading: false });
        } catch (err) {
            sidebarProvider.setState({
                chunks: [],
                total: 0,
                loading: false,
                error: `Cannot connect to ${serverUrl}`
            });
        }
    }

    context.subscriptions.push(
        vscode.commands.registerCommand("fubbik.refreshSidebar", refreshChunks)
    );

    // Create chunk command
    context.subscriptions.push(
        registerCreateChunkCommand(context, api, () => codebaseId, refreshChunks)
    );

    // Detect codebase on activation
    try {
        const result = await detectCodebase(api);
        if (result) {
            codebaseId = result.id;
            sidebarProvider.setState({ codebaseName: result.name });
        } else {
            sidebarProvider.setState({ codebaseName: null });
        }
    } catch {
        sidebarProvider.setState({ codebaseName: null });
    }

    // Fetch initial chunks
    await refreshChunks();

    // Re-detect on workspace folder change
    context.subscriptions.push(
        vscode.workspace.onDidChangeWorkspaceFolders(async () => {
            try {
                const result = await detectCodebase(api);
                codebaseId = result?.id ?? null;
                sidebarProvider.setState({ codebaseName: result?.name ?? null });
            } catch {
                codebaseId = null;
                sidebarProvider.setState({ codebaseName: null });
            }
            await refreshChunks();
        })
    );
}

export function deactivate() {}
```

- [ ] **Step 2: Build the extension**

Run: `cd apps/vscode && pnpm install && node esbuild.mjs`
Expected: `dist/extension.js` created without errors

- [ ] **Step 3: Commit**

```bash
git add apps/vscode/src/extension.ts
git commit -m "feat(vscode): add extension entry point wiring sidebar, commands, and detection"
```

---

### Task 8: Test the extension manually

- [ ] **Step 1: Build**

Run: `cd apps/vscode && node esbuild.mjs`
Expected: Build succeeds

- [ ] **Step 2: Launch in Extension Development Host**

Open the fubbik project in VS Code. Press F5 (or use the "Run Extension" launch config). A new VS Code window opens with the extension loaded.

Verify:
- Fubbik icon appears in the activity bar
- Sidebar shows chunks (or "Cannot connect" if server isn't running)
- Right-click selected text → "Add to Fubbik" appears
- Command palette → "Fubbik: Add to Knowledge Base" works
- Creating a chunk shows success notification and refreshes sidebar

- [ ] **Step 3: Commit any fixes**

```bash
git add -A && git commit -m "fix(vscode): resolve issues found during manual testing"
```
