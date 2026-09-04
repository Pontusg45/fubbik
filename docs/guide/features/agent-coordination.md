---
tags:
    - guide
    - plans
    - agents
    - mcp
description: Persistent Plan taskboards and journals for collaborating agents
---

# Agent Coordination Boards

Every Plan has a persistent coordination board. A root agent and its sub-agents can reconnect to the board, claim tasks with expiring leases, and exchange durable journal entries.

## Typical workflow

1. The root calls `join_board` with a stable `externalKey` and keeps the returned `runId`.
2. A child joins the same Plan with its own `externalKey` and the root's `runId` as `parentRunId`.
3. The child calls `claim_task` before starting a task.
4. During work, it calls `write_board_entry` for progress, questions, decisions, or artifacts.
5. It calls `update_board_task` with a retry-stable `clientMutationId` to change status and leave a completion or handoff note.
6. Other agents call `read_board` with the previous `nextSequence`, process new entries, then call `ack_board`.

Joining again with the same Plan and `externalKey` returns the original run. Reusing that key with a different handle or parent is rejected.

## Claims and leases

A task has at most one active claim. Claims default to ten minutes and may be configured from 60 seconds to one hour. Long-running agents should renew before expiry. After expiry, another run may atomically take over the task without a cleanup job.

Completing or skipping a task through `update_board_task` releases its claim. Human task controls in the Plan UI remain an administrative override.

## Journal and cursors

Journal entries are append-only and ordered by a database sequence. They may be board-wide, task-scoped, directly addressed to a run, or threaded as replies.

Use a unique `clientMutationId` for each write and reuse it if the response is lost. An identical retry returns the original result; reusing the ID for different content is rejected.

Agent reads see public entries plus messages they authored or received. The authenticated human Plan view can observe the full board. Reading does not acknowledge entries automatically; `ack_board` advances the durable cursor and heartbeat.

## Persistence versus delivery

The board stores and addresses messages, but it does not start or wake agent processes. Agents must poll `read_board`, or their host must steer a live agent when a new message is available. Webhooks, subscriptions, and host wake-up integration are future layers over the persisted journal.

## Authentication

The initial MCP workflow targets local Fubbik installations using the implicit development session. Remote MCP deployment needs scoped token authentication before exposing coordination endpoints outside that environment.
