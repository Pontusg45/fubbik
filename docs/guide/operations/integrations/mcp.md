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

The MCP server uses newline-delimited JSON-RPC over stdio. The Rust implementation exposes the complete retained catalog of fifty-three tools across core knowledge, context, requirements, tasks, coordination, plans, and behavioral matrices.

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
| `create_plan` | Create an implementation plan |
| `list_plans` | List plans with optional filters |
| `get_plan` | Get a plan with tasks, analysis, and requirements |
| `update_plan` | Update plan metadata or status |
| `link_requirement` | Link a requirement to a plan |
| `unlink_requirement` | Remove a requirement link from a plan |
| `add_analyze_item` | Add a plan analysis item |
| `update_analyze_item` | Update a plan analysis item |
| `delete_analyze_item` | Delete a plan analysis item |
| `add_plan_task` | Add a task to a plan |
| `update_plan_task` | Update a plan task |
| `delete_plan_task` | Delete a plan task |
| `link_plan_task_chunk` | Link a chunk to a plan task |
| `list_matrices` | List behavioral specification matrices |
| `get_matrix_view` | Get a matrix grid and computed statuses |
| `create_matrix` | Create a behavioral specification matrix |
| `add_dimension` | Add a matrix dimension |
| `add_rule` | Add a behavioral rule |
| `toggle_cell` | Toggle a rule and dimension intersection |
| `link_cell_requirement` | Link a requirement to a matrix cell |
| `link_cell_code` | Link a code reference to a matrix cell |
| `record_test_result` | Record a matrix cell test result |
| `get_rule_history` | Get a behavioral rule's history |
| `get_behaviors_for_file` | Find behavioral rules governing a file |
| `get_matrix_gaps` | List unspecified and violated matrix cells |
