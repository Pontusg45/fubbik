---
tags:
  - guide
  - integrations
  - mcp
description: AI agent integration via Model Context Protocol
---

# MCP Server

The Model Context Protocol (MCP) server exposes fubbik tools to AI agents like Claude Code, Cursor, and other MCP-compatible tools.

## Configuration

Add to your AI tool's MCP settings:

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

## Available Tools

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
