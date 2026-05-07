---
tags:
  - guide
  - integrations
  - vscode
description: VS Code extension — sidebar, file-aware surfacing, and quick-add
---

# VS Code Extension

The VS Code/Cursor extension provides a sidebar for browsing and managing chunks without leaving the editor.

## Installation

Build from `apps/vscode/`:

```bash
cd apps/vscode && node esbuild.mjs
code --extensionDevelopmentPath=./apps/vscode .
```

## Configuration

- `fubbik.serverUrl` — API server URL (default: `http://localhost:3000`)
- `fubbik.webAppUrl` — Web app URL (default: `http://localhost:3001`)

## Features

- **Sidebar** with type/tag/sort filtering
- **File-aware surfacing** — shows chunks relevant to the current file
- **Quick-add** — create notes without leaving the editor
- **Search** across all chunks
- **Status bar** showing chunk count
- **Webview panels** for chunk detail, creation, and editing
- **Commands**: search, quick-add note, open graph/dashboard in browser
