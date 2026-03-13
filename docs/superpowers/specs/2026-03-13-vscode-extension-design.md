# VS Code / Cursor Extension — Design Spec

## Overview

A VS Code / Cursor extension that integrates fubbik into the editor. Two features:

1. **Sidebar** — shows all chunks for the detected codebase in a webview panel
2. **Add to Fubbik** — create chunks from selected text via a form in an editor tab

The extension communicates with the fubbik server via HTTP. No auth required (relies on dev session). Standalone package in `apps/vscode/` — does not import from other fubbik packages.

## Project Structure

```
apps/vscode/
├── package.json          # Extension manifest (contributes, activationEvents)
├── tsconfig.json
├── .vscodeignore          # Excludes node_modules etc. from .vsix
├── esbuild.mjs            # Build script (bundles to single CJS file)
├── .vscode/
│   ├── launch.json        # F5 debug profile for Extension Development Host
│   └── tasks.json         # Build task
├── src/
│   ├── extension.ts       # activate/deactivate, register commands + sidebar
│   ├── api.ts             # Fubbik API client (fetch-based)
│   ├── detect-codebase.ts # Git remote detection (async exec)
│   ├── sidebar-provider.ts # WebviewViewProvider for the sidebar panel
│   └── create-chunk.ts    # "Add to fubbik" command handler + webview form
└── resources/
    └── icon.png
```

### Build

VS Code extensions must target CommonJS. Use `esbuild` to bundle `src/extension.ts` into a single CJS file at `dist/extension.js`. The `package.json` sets `"main": "./dist/extension.js"`. No `"type": "module"` — this package is CJS unlike the rest of the monorepo.

A `.vscodeignore` excludes `src/`, `node_modules/`, etc. from the packaged `.vsix`.

## Extension Manifest

### Contributes

**View Container:** Adds a "Fubbik" icon in the activity bar (left sidebar).

**View:** A webview view `fubbik.sidebar` inside that container.

**Commands:**
- `fubbik.addChunk` — "Fubbik: Add to Knowledge Base" (editor context menu + command palette)
- `fubbik.refreshSidebar` — "Fubbik: Refresh Chunks"
- `fubbik.openChunk` — opens a chunk in the fubbik web app in the browser

**Configuration:**
- `fubbik.serverUrl` — string, defaults to `http://localhost:3000`
- `fubbik.webAppUrl` — string, defaults to `http://localhost:3001`. Used for "open in browser" links. Separate from the API server URL since the web app runs on a different port.

**Menus:**
- `editor/context` — "Add to Fubbik" appears in right-click menu. Uses `"when": "editorHasSelection"` to only show when text is selected. The command palette version works without a selection.

### Activation

`onStartupFinished` — the extension activates once VS Code is ready. On activation it detects the codebase via git remote and fetches chunks for the sidebar.

## Codebase Detection

Same logic as the CLI's `detect-codebase.ts`, with async execution:

1. Guard: if `vscode.workspace.workspaceFolders` is undefined/empty, skip detection and show "No codebase detected" immediately
2. Run `git remote get-url origin` in the first workspace folder using `child_process.exec` (async, not `execSync` — avoids blocking the extension host)
3. Call `GET /api/codebases/detect?remoteUrl=<url>` on the fubbik server
4. If no git remote, fall back to `GET /api/codebases/detect?localPath=<workspaceFolderPath>` (same fallback the CLI uses)
5. If matched, store the codebase `{ id, name }` for the session
6. If no match, operate without a codebase (sidebar shows "No codebase detected", chunks created as global)

No URL normalization in the extension — the server normalizes on its end via the detect endpoint.

**Multi-root workspaces:** Uses the first workspace folder for detection. If multiple roots exist, only the first is considered.

## Sidebar Webview

The `SidebarProvider` implements `WebviewViewProvider` and renders a small HTML app.

### Content

- Header showing the detected codebase name (or "No codebase detected")
- Loading state while chunks are being fetched
- Search/filter input to narrow the chunk list client-side (filters by title, instant)
- Scrollable list of chunks, each showing:
  - Title (clickable — triggers `fubbik.openChunk` which opens `${webAppUrl}/chunks/${id}` in browser)
  - Type badge (note, document, reference, schema, checklist)
  - Created date
- When more than 100 chunks exist, show a notice: "Showing first 100 chunks"
- Empty state: "No chunks found" or "Connect to a fubbik server" with the configured URL if unreachable

### HTML Generation

The webview HTML is generated dynamically inside `SidebarProvider.resolveWebviewView()` — **not** loaded from a static file. This is required for Content Security Policy compliance: each render generates a unique nonce injected into `<script>` and `<style>` tags, with a CSP meta tag:

```html
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'nonce-${nonce}'; script-src 'nonce-${nonce}';">
```

The same approach applies to the "Add to Fubbik" webview panel.

### Data Flow

1. Extension detects codebase on activation
2. Fetches chunks: `GET /api/chunks?codebaseId=<id>&limit=100`
3. Sends chunk data to the webview via `webview.postMessage({ type: "chunks", data })`
4. Webview initializes with `const vscode = acquireVsCodeApi()` (called exactly once), renders the list with vanilla HTML/CSS/JS (no framework)
5. When a chunk is clicked, webview posts a message back: `vscode.postMessage({ type: "openChunk", id })`
6. Extension receives the message and opens `vscode.env.openExternal(Uri.parse(url))`

### Styling

Uses VS Code's built-in CSS variables for native theme matching:
- `--vscode-editor-background`
- `--vscode-editor-foreground`
- `--vscode-badge-background`
- `--vscode-input-background`
- `--vscode-input-border`
- etc.

Works in both light and dark themes without custom theme detection.

### Refresh

- Auto-refreshes when the workspace folder changes (`vscode.workspace.onDidChangeWorkspaceFolders`)
- Manual refresh via `fubbik.refreshSidebar` command (refresh button in sidebar header)
- After creating a chunk via "Add to Fubbik", the sidebar refreshes automatically

## "Add to Fubbik" Command

### Trigger

- Right-click selected text → "Add to Fubbik" in context menu (only shown when text is selected)
- Command palette → "Fubbik: Add to Knowledge Base" (works with or without selection)

### Flow

1. Get selected text from the active editor (empty string if no selection)
2. Open a **Webview panel** (editor tab, not the sidebar) with a form:
   - **Title** — text input, pre-filled with first line of selection
   - **Content** — textarea, pre-filled with full selection
   - **Type** — dropdown select (note, document, reference, schema, checklist)
   - **Tags** — comma-separated text input
   - **Submit / Cancel** buttons
3. Form HTML generated dynamically with nonce-based CSP (same as sidebar)
4. Form styled with VS Code CSS variables
5. On submit, webview posts form data to extension via `postMessage`
6. Extension calls `POST /api/chunks` with:
   ```json
   {
     "title": "...",
     "content": "...",
     "type": "...",
     "tags": ["...", "..."],
     "codebaseIds": ["<detected-codebase-id>"]
   }
   ```
7. On success: show VS Code notification "Chunk created: {title}", close the webview panel, refresh sidebar
8. On error: show error notification, keep form open

### Edge Cases

- **No text selected:** Form opens with empty content — useful for creating chunks from scratch
- **No codebase detected:** Chunk created as global (no `codebaseIds`). A note in the form: "No codebase detected — chunk will be global."
- **No workspace folder:** Same as no codebase — works in global mode
- **Server unreachable:** Error notification with the configured server URL

## API Client

Simple fetch-based client in `api.ts`:

```typescript
class FubbikApi {
  constructor(private serverUrl: string) {}

  async detectCodebase(params: { remoteUrl?: string; localPath?: string }): Promise<{ id: string; name: string } | null>
  async getChunks(codebaseId?: string): Promise<{ chunks: Chunk[]; total: number }>
  async createChunk(body: CreateChunkBody): Promise<Chunk>
}
```

Reads `fubbik.serverUrl` from VS Code configuration. No auth headers — relies on dev session.

## Future Considerations (Out of Scope)

- Auth token support for production use
- Semantic search per-file (embedding-based chunk relevance)
- Inline CodeLens annotations showing chunk references
- Edit chunks from within the editor
- Publish to VS Code Marketplace / Open VSX
