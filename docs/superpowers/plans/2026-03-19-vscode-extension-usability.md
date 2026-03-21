# VS Code Extension Usability Improvements Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve the VS Code extension from a basic sidebar viewer to a productive in-editor knowledge tool with filtering, editing, file-aware surfacing, more commands, and a status bar indicator.

**Architecture:** All changes in `apps/vscode/src/`. The extension uses VS Code's webview API for the sidebar and panels, and communicates with the fubbik server via HTTP (FubbikApi class). No backend changes needed — all endpoints already exist.

**Tech Stack:** VS Code Extension API, TypeScript, webview HTML/CSS/JS, FubbikApi (fetch-based)

---

## File Structure

### New files to create:
- `apps/vscode/src/edit-chunk.ts` — Webview panel for editing chunks
- `apps/vscode/src/status-bar.ts` — Status bar item showing chunk count
- `apps/vscode/src/file-chunks.ts` — File-aware chunk surfacing logic
- `apps/vscode/src/__tests__/api.test.ts` — Tests for API client

### Files to modify:
- `apps/vscode/src/extension.ts` — Register new commands, status bar, file change listeners
- `apps/vscode/src/sidebar-provider.ts` — Add type/tag filtering, sorting to sidebar
- `apps/vscode/src/api.ts` — Add updateChunk, getTags, getFileRefLookup, searchChunks methods
- `apps/vscode/src/chunk-detail.ts` — Add "Edit" button that opens edit panel
- `apps/vscode/package.json` — Register new commands, keybindings, configuration

---

## Task 1: Sidebar Filtering (Type and Tag)

Add dropdown filters for chunk type and tags above the chunk list in the sidebar.

**Files:**
- Modify: `apps/vscode/src/sidebar-provider.ts` (~lines 79-132 buildBody, 134-161 buildScript)

- [ ] **Step 1: Extend SidebarState with filter fields**

In `sidebar-provider.ts`, add to the state interface:
```ts
interface SidebarState {
  codebaseName: string | null;
  chunks: Chunk[];
  total: number;
  error: string | null;
  loading: boolean;
  filterType: string;  // new
  filterTag: string;   // new
  sortBy: string;      // new: "newest" | "oldest" | "title"
}
```

- [ ] **Step 2: Add filter HTML to buildBody**

In `buildBody()` (~line 79), add filter controls between the search input and the chunk list:
```html
<div class="filters">
  <select id="filter-type" class="filter-select">
    <option value="">All types</option>
    <option value="note">Note</option>
    <option value="document">Document</option>
    <option value="reference">Reference</option>
    <option value="schema">Schema</option>
    <option value="checklist">Checklist</option>
  </select>
  <select id="filter-tag" class="filter-select">
    <option value="">All tags</option>
    ${this.state.tags?.map(t => `<option value="${escapeAttr(t.name)}">${escapeHtml(t.name)}</option>`).join("")}
  </select>
  <select id="sort-by" class="filter-select">
    <option value="newest">Newest</option>
    <option value="oldest">Oldest</option>
    <option value="title">A-Z</option>
  </select>
</div>
```

- [ ] **Step 3: Add filter CSS**

In the `<style>` section or `webview-utils.ts`, add:
```css
.filters { display: flex; gap: 4px; margin-bottom: 8px; }
.filter-select {
  flex: 1;
  padding: 3px 6px;
  font-size: 11px;
  background: var(--vscode-input-background);
  color: var(--vscode-input-foreground);
  border: 1px solid var(--vscode-input-border);
  border-radius: 2px;
}
```

- [ ] **Step 4: Update buildScript for client-side filtering**

Extend the search filtering logic (~line 142-149) to also filter by type and tag:
```js
function applyFilters() {
  const search = document.getElementById("search-input").value.toLowerCase();
  const type = document.getElementById("filter-type").value;
  const tag = document.getElementById("filter-tag").value;
  const items = document.querySelectorAll(".list-item");

  items.forEach(item => {
    const title = item.dataset.title?.toLowerCase() || "";
    const itemType = item.dataset.type || "";
    const itemTags = item.dataset.tags || "";

    const matchSearch = !search || title.includes(search);
    const matchType = !type || itemType === type;
    const matchTag = !tag || itemTags.includes(tag);

    item.style.display = (matchSearch && matchType && matchTag) ? "" : "none";
  });
}

document.getElementById("filter-type").addEventListener("change", applyFilters);
document.getElementById("filter-tag").addEventListener("change", applyFilters);
document.getElementById("search-input").addEventListener("input", applyFilters);
```

- [ ] **Step 5: Add data attributes to chunk list items**

In the chunk rendering (~line 116-128), add data attributes:
```html
<div class="list-item" data-id="${id}" data-title="${escapeAttr(title)}" data-type="${type}" data-tags="${escapeAttr(tags.join(","))}">
```

- [ ] **Step 6: Fetch tags on sidebar load**

Add tags to the API and fetch them when the sidebar loads:
```ts
// In sidebar-provider.ts, loadChunks():
const [chunks, tags] = await Promise.all([
  this.api.getChunks(this.codebaseId),
  this.api.getTags(),
]);
this.setState({ chunks: chunks.chunks, tags, total: chunks.total });
```

- [ ] **Step 7: Test manually**

- Open sidebar → type and tag dropdowns visible
- Select "note" → only notes shown
- Select a tag → filtered to that tag
- Combine with search → intersection of all filters

- [ ] **Step 8: Commit**

```bash
git add apps/vscode/src/sidebar-provider.ts apps/vscode/src/api.ts
git commit -m "feat(vscode): add type, tag, and sort filtering to sidebar"
```

---

## Task 2: Inline Chunk Editing

Add an "Edit" button on chunk detail that opens a webview form for editing.

**Files:**
- Create: `apps/vscode/src/edit-chunk.ts`
- Modify: `apps/vscode/src/chunk-detail.ts` — add Edit button
- Modify: `apps/vscode/src/api.ts` — add updateChunk method
- Modify: `apps/vscode/src/extension.ts` — register edit command

- [ ] **Step 1: Add updateChunk to API client**

In `api.ts`, add:
```ts
async updateChunk(id: string, body: Partial<CreateChunkBody>): Promise<Chunk> {
  const res = await fetch(`${this.serverUrl}/api/chunks/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`Failed to update chunk: ${res.statusText}`);
  return res.json();
}
```

- [ ] **Step 2: Create edit-chunk.ts**

Model after `create-chunk.ts` but pre-fills from existing chunk data:

```ts
// apps/vscode/src/edit-chunk.ts
import * as vscode from "vscode";
import { FubbikApi, Chunk } from "./api";
import { getBaseHtml, getNonce } from "./webview-utils";

export function showEditChunk(
  api: FubbikApi,
  chunk: Chunk,
  onUpdated: () => void,
) {
  const panel = vscode.window.createWebviewPanel(
    "fubbikEditChunk",
    `Edit: ${chunk.title}`,
    vscode.ViewColumn.One,
    { enableScripts: true },
  );

  const nonce = getNonce();
  const body = buildEditForm(chunk);
  const script = buildEditScript(nonce);
  // NOTE: getBaseHtml takes positional args: (webview, nonce, body, script)
  panel.webview.html = getBaseHtml(panel.webview, nonce, body, script);

  panel.webview.onDidReceiveMessage(async (msg) => {
    if (msg.type === "submit") {
      try {
        // NOTE: Server expects `source` field for chunk type, not `type`
        await api.updateChunk(chunk.id, msg.data);
        vscode.window.showInformationMessage("Chunk updated");
        panel.dispose();
        onUpdated();
      } catch (err: any) {
        vscode.window.showErrorMessage(err.message);
      }
    }
    if (msg.type === "cancel") panel.dispose();
  });
}

// NOTE: Must define escapeHtml/escapeAttr locally (each file in the extension has its own copy)
function escapeHtml(str: string): string {
  return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
function escapeAttr(str: string): string {
  return escapeHtml(str).replace(/"/g, "&quot;");
}

function buildEditForm(chunk: Chunk): string {
  return `
    <h2>Edit Chunk</h2>
    <label>Title</label>
    <input id="title" type="text" value="${escapeAttr(chunk.title)}" />
    <label>Content</label>
    <textarea id="content" rows="15">${escapeHtml(chunk.content)}</textarea>
    <label>Type</label>
    <select id="type">
      ${["note","document","reference","schema","checklist"].map(t =>
        `<option value="${t}" ${t === chunk.source ? "selected" : ""}>${t}</option>`
      ).join("")}
    </select>
    <label>Tags (comma-separated)</label>
    <input id="tags" type="text" value="${escapeAttr((chunk.tags || []).join(", "))}" />
    <div class="mt-3">
      <button id="submit-btn">Save</button>
      <button id="cancel-btn" class="secondary">Cancel</button>
    </div>
  `;
}

// NOTE: In buildEditScript, the submit message must send `source` (not `type`) for the chunk type field:
// vscode.postMessage({ type: "submit", data: { title, content, source: typeValue, tags } })
```

- [ ] **Step 3: Add Edit button to chunk detail**

In `chunk-detail.ts` (~lines 57-60), add an "Edit" button alongside "Open in Browser":
```html
<button id="edit-btn">Edit</button>
```

Handle the message:
```ts
if (msg.type === "edit") {
  showEditChunk(api, chunk, () => {
    // Refresh the detail panel — showChunkDetail takes (api, chunkId) only (reads webAppUrl from config internally)
    showChunkDetail(api, chunk.id);
  });
}
```

- [ ] **Step 4: Test manually**

- Open chunk detail → click Edit → form opens with pre-filled data
- Change title → click Save → detail refreshes with new title
- Click Cancel → returns to detail

- [ ] **Step 5: Commit**

```bash
git add apps/vscode/src/edit-chunk.ts apps/vscode/src/chunk-detail.ts apps/vscode/src/api.ts
git commit -m "feat(vscode): add inline chunk editing from detail panel"
```

---

## Task 3: File-Aware Chunk Surfacing

Show chunks related to the currently active file at the top of the sidebar.

**Files:**
- Create: `apps/vscode/src/file-chunks.ts`
- Modify: `apps/vscode/src/api.ts` — add getFileRefLookup method
- Modify: `apps/vscode/src/sidebar-provider.ts` — add "Related to this file" section
- Modify: `apps/vscode/src/extension.ts` — listen for active editor changes

- [ ] **Step 1: Add getFileRefLookup to API**

```ts
// In api.ts
async getFileRefLookup(path: string): Promise<Chunk[]> {
  const res = await fetch(`${this.serverUrl}/api/file-refs/lookup?path=${encodeURIComponent(path)}`);
  if (!res.ok) return [];
  return res.json();
}
```

- [ ] **Step 2: Create file-chunks.ts utility**

```ts
// apps/vscode/src/file-chunks.ts
import * as vscode from "vscode";
import { FubbikApi, Chunk } from "./api";

export async function getChunksForFile(api: FubbikApi, uri: vscode.Uri): Promise<Chunk[]> {
  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (!workspaceFolder) return [];

  // Get relative path from workspace root
  const relativePath = vscode.workspace.asRelativePath(uri);
  return api.getFileRefLookup(relativePath);
}
```

- [ ] **Step 3: Add "Related to this file" section to sidebar**

In `sidebar-provider.ts`, add a section above the main chunk list:
```html
${fileChunks.length > 0 ? `
  <div class="section-header">Related to current file</div>
  ${fileChunks.map(c => `<div class="list-item highlighted" data-id="${c.id}">...`).join("")}
  <hr />
` : ""}
```

- [ ] **Step 4: Listen for active editor changes**

In `extension.ts`, add:
```ts
vscode.window.onDidChangeActiveTextEditor(async (editor) => {
  if (editor) {
    const fileChunks = await getChunksForFile(api, editor.document.uri);
    sidebarProvider.setFileChunks(fileChunks);
  }
});
```

- [ ] **Step 5: Commit**

```bash
git add apps/vscode/src/file-chunks.ts apps/vscode/src/sidebar-provider.ts apps/vscode/src/api.ts apps/vscode/src/extension.ts
git commit -m "feat(vscode): surface chunks related to active file in sidebar"
```

---

## Task 4: Additional Commands

Register more commands in the command palette for common operations.

**Files:**
- Modify: `apps/vscode/src/extension.ts`
- Modify: `apps/vscode/package.json`

- [ ] **Step 1: Add commands to package.json**

In the `contributes.commands` array:
```json
{ "command": "fubbik.searchChunks", "title": "Fubbik: Search Chunks" },
{ "command": "fubbik.openGraph", "title": "Fubbik: Open Graph in Browser" },
{ "command": "fubbik.openDashboard", "title": "Fubbik: Open Dashboard in Browser" },
{ "command": "fubbik.quickAddNote", "title": "Fubbik: Quick Add Note" }
```

- [ ] **Step 2: Implement searchChunks command**

Uses VS Code QuickPick for search:
```ts
vscode.commands.registerCommand("fubbik.searchChunks", async () => {
  const quickPick = vscode.window.createQuickPick();
  quickPick.placeholder = "Search chunks...";
  quickPick.onDidChangeValue(async (query) => {
    if (query.length < 2) return;
    // NOTE: getChunks only accepts codebaseId. Must extend api.ts to add a search param,
    // or add a new searchChunks(query, codebaseId) method that appends ?search=query to the URL.
    const res = await api.searchChunks(query, codebaseId);
    quickPick.items = res.chunks.map((c: any) => ({
      label: c.title,
      description: `[${c.source}] ${(c.tags || []).join(", ")}`,
      detail: c.content?.slice(0, 100) ?? "",
      chunkId: c.id,
    }));
  });
  quickPick.onDidAccept(() => {
    const selected = quickPick.selectedItems[0] as any;
    if (selected?.chunkId) showChunkDetail(api, selected.chunkId);
    quickPick.dispose();
  });
  quickPick.show();
});
```

- [ ] **Step 3: Implement browser-opening commands**

```ts
vscode.commands.registerCommand("fubbik.openGraph", () => {
  vscode.env.openExternal(vscode.Uri.parse(`${webAppUrl}/graph`));
});

vscode.commands.registerCommand("fubbik.openDashboard", () => {
  vscode.env.openExternal(vscode.Uri.parse(`${webAppUrl}/dashboard`));
});
```

- [ ] **Step 4: Implement quickAddNote**

Prompts for title only, creates a note with minimal friction:
```ts
vscode.commands.registerCommand("fubbik.quickAddNote", async () => {
  const title = await vscode.window.showInputBox({ prompt: "Chunk title" });
  if (!title) return;
  const content = await vscode.window.showInputBox({ prompt: "Content (optional)" });
  await api.createChunk({ title, content: content || "", source: "note", tags: undefined, codebaseId });
  vscode.window.showInformationMessage(`Created: ${title}`);
  // NOTE: SidebarProvider has no refresh() method. Use the registered command instead:
  vscode.commands.executeCommand("fubbik.refreshSidebar");
});
```

- [ ] **Step 5: Add keybindings**

In `package.json` contributes section:
```json
"keybindings": [
  { "command": "fubbik.searchChunks", "key": "ctrl+shift+f", "mac": "cmd+shift+f", "when": "fubbik.active" },
  { "command": "fubbik.quickAddNote", "key": "ctrl+shift+n", "mac": "cmd+shift+n", "when": "fubbik.active" }
]
```

- [ ] **Step 6: Commit**

```bash
git add apps/vscode/src/extension.ts apps/vscode/package.json
git commit -m "feat(vscode): add search, quick-add, and browser commands"
```

---

## Task 5: Status Bar Indicator

Show chunk count for current codebase in the VS Code status bar.

**Files:**
- Create: `apps/vscode/src/status-bar.ts`
- Modify: `apps/vscode/src/extension.ts`

- [ ] **Step 1: Create status-bar.ts**

```ts
// apps/vscode/src/status-bar.ts
import * as vscode from "vscode";
import { FubbikApi } from "./api";

export class FubbikStatusBar {
  private item: vscode.StatusBarItem;

  constructor() {
    this.item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    this.item.command = "fubbik.sidebar.focus";
    this.item.tooltip = "Fubbik Knowledge Base";
  }

  async update(api: FubbikApi, codebaseId?: string) {
    try {
      const res = await api.getChunks(codebaseId);
      this.item.text = `$(book) ${res.total} chunks`;
      this.item.show();
    } catch {
      this.item.text = "$(book) Fubbik";
      this.item.color = new vscode.ThemeColor("errorForeground");
      this.item.tooltip = "Cannot connect to Fubbik server";
      this.item.show();
    }
  }

  dispose() {
    this.item.dispose();
  }
}
```

- [ ] **Step 2: Integrate in extension.ts**

```ts
const statusBar = new FubbikStatusBar();
context.subscriptions.push(statusBar);

// Update on activation and after chunk operations
statusBar.update(api, codebaseId);
```

- [ ] **Step 3: Update status bar after chunk create/refresh**

After each `refreshSidebar` or `addChunk` operation, call `statusBar.update(api, codebaseId)`.

- [ ] **Step 4: Commit**

```bash
git add apps/vscode/src/status-bar.ts apps/vscode/src/extension.ts
git commit -m "feat(vscode): add status bar indicator showing chunk count"
```
