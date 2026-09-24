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
            "command": "fubbik",
            "args": ["mcp"],
            "env": {
                "FUBBIK_SERVER_URL": "http://localhost:3100"
            }
        }
    }
}
```

The MCP server uses newline-delimited JSON-RPC over stdio. The Rust migration currently exposes twenty-eight core, context, requirement, task, and coordination tools. Plan and matrix tools are being moved in subsequent migration slices and remain available through the legacy development package until their contract tests pass.

## Available Tools

| Tool                   | Description                           |
| ---------------------- | ------------------------------------- |
| `search_chunks`        | Search knowledge by text or semantics |
| `get_chunk`            | Get full chunk details                |
| `create_chunk`         | Create a new chunk                    |
| `get_conventions`      | Get coding conventions for a file     |
| `get_requirements`     | List requirements                     |
| `search_vocabulary`    | Search controlled vocabulary          |
| `update_chunk`         | Update an existing chunk              |
| `list_updates`         | List tagged knowledge updates         |
| `propose_chunk_update` | Propose an update for review           |
| `sync_claude_md`       | Generate CLAUDE.md content             |
| `get_context`          | Retrieve plan, concept, or file context |
| `get_context_for_task` | Retrieve context scoped to a plan task |
| `create_context_snapshot` | Freeze context into a snapshot      |
| `get_context_snapshot` | Retrieve frozen context                |
| `list_requirements` | Filter requirements by status, priority, space, or text |
| `create_requirement` | Create a requirement with Given/When/Then steps |
| `update_requirement_status` | Mark a requirement passing, failing, or untested |
| `suggest_requirements` | Build suggestion context from requirements and knowledge health |
| `create_requirements_batch` | Create approved requirements and resolve use cases |
| `add_task` | Add a quick task backed by a single-task plan |
| `list_tasks` | List in-progress quick tasks |
| `complete_task` | Complete a quick task |
| `join_board` | Join or reconnect to a persistent plan board |
| `read_board` | Read board tasks, claims, agents, and journal entries |
| `claim_task` | Claim, renew, or release a task lease |
| `update_board_task` | Transition a task held by an agent run |
| `write_board_entry` | Write a durable board note or message |
| `ack_board` | Persist a run's journal cursor and heartbeat |
