---
tags:
  - guide
  - integrations
description: MCP server, VS Code extension, and AI tool integration
---

# Integrations

Fubbik integrates with AI tools and editors so your knowledge base is available where you work.

## MCP Server

The Model Context Protocol (MCP) server exposes fubbik tools to AI agents like Claude Code, Cursor, and other MCP-compatible tools.

Configure in your AI tool's MCP settings:

```json
{
  "mcpServers": {
    "fubbik": {
      "command": "npx",
      "args": ["tsx", "packages/mcp/src/index.ts"],
      "env": {
        "FUBBIK_SERVER_URL": "http://localhost:3000"
      }
    }
  }
}
```

Available MCP tools:

| Tool | Description |
|------|-------------|
| `search_chunks` | Search knowledge by text or semantics |
| `get_chunk` | Get full chunk details |
| `create_chunk` | Create a new chunk |
| `get_conventions` | Get coding conventions for a file |
| `get_requirements` | List requirements |
| `search_vocabulary` | Search controlled vocabulary |
| `create_plan` | Create an implementation plan |
| `begin_implementation` | Start an implementation session |
| `mark_plan_step` | Update a plan step status |
| `sync_claude_md` | Regenerate CLAUDE.md |

## VS Code Extension

The VS Code/Cursor extension provides a sidebar for browsing and managing chunks without leaving the editor.

Install from `apps/vscode/`:

```bash
cd apps/vscode && node esbuild.mjs
code --extensionDevelopmentPath=./apps/vscode .
```

Configure in VS Code settings:
- `fubbik.serverUrl` — API server URL (default: `http://localhost:3000`)
- `fubbik.webAppUrl` — Web app URL (default: `http://localhost:3001`)

Features:
- **Sidebar** with type/tag/sort filtering
- **File-aware surfacing** — shows chunks relevant to the current file
- **Quick-add** — create notes without leaving the editor
- **Search** across all chunks
- **Status bar** showing chunk count
- **Webview panels** for chunk detail, creation, and editing

## Ollama (Local AI)

Fubbik uses Ollama for local AI features. Install Ollama and pull the required models:

```bash
ollama pull nomic-embed-text   # Embedding model (768-dim vectors)
ollama pull llama3.2           # Generation model for enrichment
```

Set `OLLAMA_URL` in your environment (default: `http://localhost:11434`).

Ollama powers:
- **Chunk enrichment** — auto-generated summaries, aliases, and "not about" terms
- **Semantic search** — find chunks by meaning, not just keywords
- **Duplicate detection** — warns when creating chunks similar to existing ones

All features work without Ollama — you just lose the AI-powered extras.

## API

The fubbik API is a REST API built with Elysia. Full OpenAPI/Swagger docs are available at `http://localhost:3000/docs`.

Key endpoints:

```
GET    /api/chunks                    # List chunks
POST   /api/chunks                    # Create chunk
GET    /api/chunks/:id                # Get chunk detail
GET    /api/chunks/search/semantic    # Semantic search
GET    /api/chunks/export/context     # Token-budgeted export
GET    /api/context/for-file          # Chunks relevant to a file
GET    /api/documents                 # List imported documents
POST   /api/documents/import          # Import markdown as document
GET    /api/graph                     # Graph data (nodes + edges)
```
