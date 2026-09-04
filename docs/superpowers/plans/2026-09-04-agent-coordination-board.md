# Agent Coordination Board Implementation Plan

**Date:** 2026-09-04
**Status:** Implemented

**Goal:** Let a root agent and its sub-agents coordinate through a durable Plan-backed taskboard: agents can reconnect, claim work without races, leave addressed or shared notes, hand work back, and incrementally read everything that changed while they were away.

**Architecture:** Keep `plan` and `plan_task` as the existing unit-of-work and taskboard models. Add a `coordination` module beside the plans module with three persisted concepts: agent runs, leased task claims, and an append-only coordination journal. The Rust database/API path owns persistence and invariants; the TypeScript MCP package is a thin HTTP adapter. The existing plan page gains a read-only coordination view and claim badges. Durable storage does not itself wake a live sub-agent; polling is the MVP delivery mechanism.

**Tech Stack:** PostgreSQL 18, Rust 2024, sqlx 0.8, axum 0.8, utoipa 5, TypeScript MCP SDK, React 19, TanStack Query.

## Product Contract

### Terminology

- **Board:** the coordination projection for one existing Plan.
- **Agent run:** a reconnectable identity for one agent participating in the Plan. Runs are not permanent personas.
- **Claim:** an exclusive, expiring lease held by an agent run on one Plan task.
- **Entry:** an immutable journal item. Entries may be board-wide, task-scoped, directly addressed, or any combination of those.
- **Cursor:** the largest journal sequence an agent has acknowledged.

### Required behavior

1. Joining with a new `(plan_id, external_key)` creates a run; joining again with the same pair and identity fields returns the existing run. Reusing the key with a different handle or parent is a conflict.
2. Parent and child runs must belong to the same Plan.
3. At most one unexpired claim may exist for a task.
4. Claim acquisition and expired-claim takeover must be atomic under concurrent requests.
5. The current holder can renew or release its claim. Another run cannot do either.
6. A claimed task can be transitioned through the coordination interface only by its current holder. Existing human Plan endpoints remain an administrative override.
7. Completing or skipping a task through the coordination interface releases its claim in the same transaction.
8. Entries are append-only and receive a monotonically increasing sequence.
9. Repeating a write with the same `(author_run_id, client_mutation_id)` returns the original entry rather than creating a duplicate.
10. Board sync always returns current tasks/runs/claims plus journal entries after the supplied cursor, ordered by `(sequence ASC)`.
11. Acknowledgement cursors only move forward.
12. Every database operation is scoped through the owning Plan's `user_id`; cross-user IDs return not-found and never mutate data.
13. Agent reads see board-wide entries plus entries they authored or received; they do not see direct messages between other runs. A human observer may see the entire user-owned board.
14. Task transitions are retry-safe by `client_mutation_id`, including a repeated terminal transition after its first attempt released the claim.

### Explicit non-goals for the MVP

- Starting, stopping, or waking agent processes.
- WebSockets, Server-Sent Events, webhooks, or MCP subscriptions. Agents poll `read_board`.
- Multiple simultaneous assignees on one task.
- Attachments or binary artifacts. Entries link to files, chunks, commits, and URLs through Markdown/metadata.
- Cryptographic isolation between cooperative agents under the same Fubbik user. `runId` identifies authorship but is not a per-agent security capability.
- Implementing the native Rust `fubbik mcp` command. The existing TypeScript MCP package remains the consumer.
- Adding API-token authentication. The existing implicit local session remains sufficient for the first slice; remote MCP authentication is a follow-up prerequisite for deployment.

## Persisted Model

### `agent_run`

| Column | Type / constraint | Purpose |
| --- | --- | --- |
| `id` | text PK | Public run identifier |
| `plan_id` | text FK → `plan`, cascade | Board membership |
| `parent_run_id` | nullable text | Parent run; composite FK keeps it on the same Plan |
| `handle` | text, non-empty | Human-readable role/name |
| `external_key` | nullable text | Caller-stable reconnect key |
| `status` | text, check `active/finished/abandoned` | Lifecycle label |
| `capabilities` | jsonb default `[]` | Informational capability names |
| `metadata` | jsonb default `{}` | Host/thread/model data without schema churn |
| `last_ack_sequence` | bigint default `0` | Durable inbox cursor |
| `last_heartbeat_at` | timestamp | Liveness hint, not a lock |
| `created_at`, `updated_at` | timestamp | Audit fields |

Constraints/indexes:

- Unique `(id, plan_id)` to support same-Plan composite foreign keys.
- Partial unique `(plan_id, external_key) WHERE external_key IS NOT NULL`.
- Index `(plan_id, status)` and `(parent_run_id)`.
- Composite FK `(parent_run_id, plan_id) → agent_run(id, plan_id)`.

### `plan_task_claim`

| Column | Type / constraint | Purpose |
| --- | --- | --- |
| `task_id` | text PK | Exactly one claim row per task |
| `plan_id` | text | Used by same-Plan composite constraints |
| `agent_run_id` | text | Current holder |
| `claimed_at` | timestamp | First acquisition time |
| `lease_expires_at` | timestamp | Crash recovery / takeover threshold |
| `updated_at` | timestamp | Renewal time |

Add `UNIQUE (id, plan_id)` to `plan_task`, then enforce:

- `(task_id, plan_id) → plan_task(id, plan_id)` with cascade.
- `(agent_run_id, plan_id) → agent_run(id, plan_id)` with cascade.
- Index `(plan_id, lease_expires_at)` for board reads and stale-claim cleanup.

An expired row may remain stored; board reads distinguish active/expired using database `now()`, and a later claimant replaces it atomically. No background worker is required.

### `coordination_entry`

| Column | Type / constraint | Purpose |
| --- | --- | --- |
| `id` | text PK | Public entry identifier |
| `sequence` | bigint identity, unique | Stable incremental cursor |
| `plan_id` | text FK → `plan`, cascade | Board membership |
| `task_id` | nullable text | Optional task scope |
| `author_run_id` | text | Agent authorship |
| `recipient_run_id` | nullable text | Direct recipient; null means board-visible |
| `reply_to_id` | nullable text FK → self, cascade | Threading hint; composite FK keeps replies on the same Plan |
| `kind` | text | `note/question/answer/progress/decision/handoff/artifact/system` |
| `body` | text, non-empty | Markdown payload |
| `metadata` | jsonb default `{}` | File paths, chunk IDs, URLs, commit SHAs, etc. |
| `client_mutation_id` | text | Retry idempotency key |
| `created_at` | timestamp | Display/audit time |

Use composite foreign keys to enforce that task, author, recipient, and replied-to entry all belong to `plan_id`. Add unique `(id, plan_id)`, unique `(author_run_id, client_mutation_id)`, and indexes `(plan_id, sequence)`, `(recipient_run_id, sequence)`, and `(task_id, sequence)`.

## Public Interfaces

### HTTP

All endpoints live below `/api/plans/{planId}/board` and require `CurrentUser`.

| Method | Path | Behavior |
| --- | --- | --- |
| `POST` | `/runs` | Join or reconnect using `externalKey` |
| `GET` | `/` | Return board snapshot; accepts optional `runId`, plus `afterSequence`, `limit` |
| `POST` | `/tasks/{taskId}/claim` | `action = claim/renew/release`, default lease 10 minutes |
| `POST` | `/tasks/{taskId}/transition` | Change task status and optionally append a note atomically |
| `POST` | `/entries` | Append or idempotently replay a journal entry |
| `POST` | `/runs/{runId}/ack` | Advance cursor and heartbeat; may also set run status |

The board response is one cohesive read model:

```json
{
  "plan": { "id": "...", "title": "...", "status": "..." },
  "tasks": [{ "id": "...", "title": "...", "status": "...", "dependsOn": [] }],
  "runs": [{ "id": "...", "parentRunId": null, "handle": "root", "status": "active" }],
  "claims": [{ "taskId": "...", "agentRunId": "...", "leaseExpiresAt": "...", "expired": false }],
  "entries": [{ "sequence": 43, "kind": "handoff", "body": "..." }],
  "cursor": { "nextSequence": 43, "acknowledgedSequence": 40, "hasMore": false }
}
```

`limit` defaults to 100 and is capped at 500. An initial read uses `afterSequence=0`. Current tasks/runs/claims are returned on every read so a caller cannot miss mutable state even if its journal cursor is old. When `runId` is present, direct-message visibility is restricted to entries authored by or addressed to that run, and `acknowledgedSequence` is populated. When it is absent, the authenticated human observer sees the full board and `acknowledgedSequence` is null.

### MCP

Expose these tools from a new `coordination` plugin:

- `join_board`
- `read_board`
- `claim_task` with `action: claim | renew | release`
- `update_board_task` with a retry-stable `clientMutationId`
- `write_board_entry`
- `ack_board`

Every mutating MCP tool takes `runId`; journal writes additionally require a caller-generated `clientMutationId`. Tool output should be compact Markdown for model consumption, with IDs and the next cursor preserved verbatim. Do not retain a process-global “current run”: one MCP process may serve more than one conversation.

## Task Order

```text
Baseline
   ↓
Schema migration + Drizzle mirror
   ↓
Agent/journal repository ──→ Claim/transition repository
             └──────────────┬──────────────┘
                            ↓
                      Rust HTTP module
                            ↓
                 OpenAPI + generated client
                      ↙             ↘
                 MCP adapter       Web projection
                      ↘             ↙
                    Docs + full verification
```

## Task 0: Capture the Baseline and Protect Existing Work

**Files:** none.

- [ ] Record `git status --short` before implementation. The working tree may already contain unrelated user changes; do not reset, revert, stage, or reformat them.
- [ ] Record the current migration list and OpenAPI hash:

```bash
ls crates/fubbik-db/migrations
shasum -a 256 openapi.json
```

- [ ] Run focused existing Plan tests before touching the domain:

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
cargo test -p fubbik-db --test plan
cargo test -p fubbik-api --test plans
```

- [ ] If failures are pre-existing, record their exact names and continue only when they do not overlap the files or behavior in this plan.

## Task 1: Add the Coordination Schema

**Files:**

- Create `crates/fubbik-db/migrations/0005_agent_coordination_board.sql`
- Create `packages/db/src/schema/coordination.ts`
- Modify `packages/db/src/schema/plan.ts`
- Modify `packages/db/src/schema/index.ts`

**Produces:** the three tables and constraints in “Persisted Model,” available to both Rust migrations and the legacy Drizzle schema tooling.

- [ ] Write a failing schema-level test in `crates/fubbik-db/tests/coordination.rs` proving same-Plan parent/child, task/claim, and entry author/recipient constraints reject cross-Plan rows.
- [ ] Add migration `0005`. Do not edit `0001_init.sql`; existing databases must advance normally.
- [ ] Use a generated identity for `coordination_entry.sequence`; never derive a cursor from timestamps or UUID ordering.
- [ ] Add the composite uniqueness constraint to `plan_task` before referencing it.
- [ ] Mirror the tables and relations in Drizzle so `pnpm db:push` cannot interpret the Rust-created tables as unmanaged drift.
- [ ] Run:

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
cargo test -p fubbik-db --test coordination
pnpm --filter @fubbik/db check-types
```

**Commit boundary:** `feat(db): add agent coordination board schema`

## Task 2: Implement Agent Runs and the Journal Repository

**Files:**

- Create `crates/fubbik-db/src/repo/coordination.rs`
- Create/extend `crates/fubbik-db/tests/coordination.rs`
- Modify `crates/fubbik-db/src/repo/mod.rs`

**Produces:** typed repository interfaces for joining runs, reading board state, appending entries, and acknowledging cursors.

- [ ] Define wire-ready repository structs for `AgentRun`, `CoordinationEntry`, and board query filters using camelCase serialization and `utoipa::ToSchema` where they cross HTTP.
- [ ] Add `join_run(pool, user_id, plan_id, input)`. Use the partial unique key to make reconnect idempotent under concurrent calls. A supplied `parent_run_id` must resolve under the same user and Plan. If an existing key is paired with a different handle or parent, return a conflict instead of silently changing identity.
- [ ] Add `list_runs`, `list_entries_after`, and `max_sequence`. Order entries only by sequence.
- [ ] Add `append_entry`. Validate kind and non-empty body, resolve all referenced IDs through the user's Plan, and use `(author_run_id, client_mutation_id)` for idempotent replay. A replay with different kind/body/scope/recipient metadata is a conflict rather than a successful retry.
- [ ] Add `ack_run`. Apply `GREATEST(last_ack_sequence, requested)` and update the heartbeat in one statement. Reject acknowledgement beyond the board's current maximum sequence.
- [ ] Write tests first for:
  - reconnect returning the original run;
  - two concurrent reconnects producing one row;
  - parent/child Plan mismatch;
  - stable sequence ordering when timestamps tie;
  - idempotent entry replay returning the original ID and sequence;
  - conflicting reuse of an external key or mutation ID;
  - direct-recipient and task scoping;
  - a run cannot read direct entries exchanged between two siblings;
  - cursor monotonicity and out-of-range rejection;
  - every cross-user read/write returning no match while leaving rows intact.
- [ ] Run `cargo test -p fubbik-db --test coordination`.

**Commit boundary:** `feat(db): persist agent runs and coordination journal`

## Task 3: Implement Atomic Claims and Agent Task Transitions

**Files:**

- Modify `crates/fubbik-db/src/repo/coordination.rs`
- Extend `crates/fubbik-db/tests/coordination.rs`

**Produces:** transactional claim and transition operations; callers never implement lease logic themselves.

- [ ] Add `mutate_claim(pool, user_id, plan_id, task_id, run_id, action, lease_duration)`.
- [ ] Implement `claim` as one `INSERT ... ON CONFLICT (task_id) DO UPDATE ... WHERE` statement. It may acquire an absent claim, renew the same holder, or take over a claim whose `lease_expires_at <= now()`. It must return a typed conflict when another unexpired holder wins.
- [ ] Clamp lease duration to 60 seconds–60 minutes in the module; default to 10 minutes at the HTTP layer.
- [ ] Implement `renew` and `release` with both `task_id` and `agent_run_id` predicates.
- [ ] Add `transition_claimed_task` as one database transaction. Every request carries `client_mutation_id`:
  1. return the previously recorded result when this run already used the mutation ID for the identical transition; reject reuse with different input;
  2. lock and validate the active claim against database `now()`;
  3. validate the requested Plan task status;
  4. update the existing `plan_task` row;
  5. append one journal entry carrying the operation result: `progress` with the caller's note, or `system` with a generated status-change body;
  6. delete the claim for `done` or `skipped`;
  7. preserve the existing “done unblocks dependents” behavior.
- [ ] Return domain errors distinguishable as not-found, claim-conflict, lease-expired, and validation failures. Do not expose raw SQL constraint text.
- [ ] Write deterministic tests using explicit short/expired database timestamps rather than sleeps:
  - two concurrent claim attempts yield exactly one holder;
  - the same holder can retry/renew;
  - another run cannot renew, release, or transition;
  - an expired claim can be taken over;
  - completion releases the claim and unblocks dependent tasks;
  - retrying that completion after claim release returns its original success without a second journal row;
  - a transition plus note is all-or-nothing;
  - cross-Plan and cross-user IDs do not mutate anything.
- [ ] Run `cargo test -p fubbik-db --test coordination`.

**Commit boundary:** `feat(db): add leased task claims and atomic transitions`

## Task 4: Add the Rust Coordination HTTP Module

**Files:**

- Create `crates/fubbik-api/src/coordination/mod.rs`
- Create `crates/fubbik-api/src/coordination/dto.rs`
- Create `crates/fubbik-api/src/coordination/service.rs`
- Create `crates/fubbik-api/src/coordination/routes.rs`
- Create `crates/fubbik-api/tests/coordination.rs`
- Modify `crates/fubbik-api/src/lib.rs`
- Modify `crates/fubbik-api/src/error.rs` if new domain-error mappings are required

**Produces:** the six HTTP operations in “Public Interfaces.”

- [ ] Define DTOs with explicit serde camelCase and utoipa schemas. Use string enums for closed request vocabularies (`kind`, claim `action`, run `status`) because this is a greenfield contract, not a Node-parity port.
- [ ] Build a `BoardSnapshot` in the service by composing the existing Plan/task repository with coordination reads. Include task dependencies without making callers issue a second request.
- [ ] When `runId` is supplied, require it to belong to the current user and Plan and filter direct entries to those authored by or addressed to it. Without `runId`, treat the authenticated caller as a human observer: return all entries but no agent acknowledgement cursor.
- [ ] Map an active claim conflict to HTTP `409`; invalid cursor/lease/status/kind to `400`; foreign or missing Plan/run/task to `404`.
- [ ] Cap journal `limit` at 500 and return `hasMore` without skipping entries. Fetch `limit + 1`, return the first `limit`, and use the final returned sequence as `nextSequence`.
- [ ] A board read must not implicitly acknowledge entries. Only the ack endpoint mutates the cursor.
- [ ] Keep route handlers shallow: authentication and extraction in routes, invariants/orchestration in the module, SQL in the repository.
- [ ] Write HTTP tests for response shape, pagination, all error mappings, retry idempotency, cookie authentication, and cross-user isolation.
- [ ] Run:

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
cargo test -p fubbik-api --test coordination
cargo test -p fubbik-api --test plans
```

**Commit boundary:** `feat(api): expose Plan coordination boards`

## Task 5: Publish OpenAPI and Regenerate the Web Client

**Files:**

- Modify `crates/fubbik-api/src/openapi.rs`
- Modify `openapi.json`
- Modify `apps/web/src/utils/api-types.ts`
- Extend `crates/fubbik-api/tests/openapi.rs` or `schema_names.rs` if necessary

**Produces:** a committed, typed contract for MCP-adjacent clients and the web UI.

- [ ] Register all coordination routes and schemas in `ApiDoc`.
- [ ] Generate and inspect the spec:

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
cargo run -- openapi > openapi.json
pnpm --filter web gen:api
```

- [ ] Confirm the OpenAPI diff is confined to coordination paths/schemas and intended generated-client changes.
- [ ] Run:

```bash
cargo test -p fubbik-api --test openapi
cargo test -p fubbik-api --test schema_names
pnpm --filter web check-types
```

**Commit boundary:** `feat(api): publish coordination board contract`

## Task 6: Add the MCP Coordination Adapter

**Files:**

- Create `packages/mcp/src/coordination-tools.ts`
- Create `packages/mcp/src/coordination-format.ts`
- Create `packages/mcp/src/coordination-format.test.ts`
- Modify `packages/mcp/src/index.ts`
- Modify `packages/mcp/package.json`

**Produces:** the six MCP tools in “Public Interfaces,” backed only by HTTP calls.

- [ ] Add a `coordinationPlugin` and register it in `index.ts`.
- [ ] Keep HTTP response types local and explicit; do not introduce `any` for board structures.
- [ ] Make `read_board` output scan-friendly Markdown grouped as tasks, agents, direct messages, and journal updates. Preserve full IDs, sequence numbers, lease expiry, `hasMore`, and `nextSequence`.
- [ ] Tell agents in tool descriptions that:
  - `externalKey` is how they reconnect;
  - they must retain `runId` and cursor in their working context;
  - `clientMutationId` must be stable across retries;
  - direct entries persist but do not wake the recipient;
  - claim leases need renewal during long work.
- [ ] Do not store a global current Plan/run/cursor in the MCP server.
- [ ] Add Vitest to the package's development/test scripts and unit-test formatting, pagination warnings, expired-claim rendering, and preservation of full identifiers.
- [ ] Run:

```bash
pnpm --filter @fubbik/mcp test
pnpm exec tsc -p packages/mcp/tsconfig.json --noEmit
```

**Commit boundary:** `feat(mcp): add persistent agent coordination tools`

## Task 7: Surface the Board in the Existing Plan UI

**Files:**

- Create `apps/web/src/features/plans/plan-coordination-panel.tsx`
- Create `apps/web/src/features/plans/plan-coordination-panel.test.tsx`
- Modify `apps/web/src/features/plans/plan-task-card.tsx`
- Modify `apps/web/src/features/plans/plan-tasks-section.tsx`
- Modify `apps/web/src/routes/plans.$planId.tsx`

**Produces:** a human-readable projection of agent participation and journal activity without creating a second task-management page.

- [ ] Fetch the board without `runId`, using the authenticated human-observer behavior from Task 4 rather than impersonating an agent run.
- [ ] Poll every 5 seconds while the page is visible. Pause when `document.visibilityState !== "visible"`; refetch immediately on visibility restoration.
- [ ] Add the active claimant's handle and lease state to each existing task card. Keep existing human task controls unchanged as administrative overrides.
- [ ] Replace or supplement the activity sidebar with tabs for `Activity`, `Agents`, and `Journal`; do not duplicate the existing task list into a second kanban.
- [ ] Render direct-message recipients, task links, entry kinds, timestamps, and expired/stale indicators accessibly. Do not rely on color alone.
- [ ] Keep the MVP journal read-only for humans. Human-authored messages require a separate authorship model and are deliberately deferred.
- [ ] Test empty state, active/expired claims, nested parent/child runs, direct messages, pagination notice, and polling visibility behavior.
- [ ] Run:

```bash
pnpm --filter web test -- plan-coordination-panel
pnpm --filter web check-types
pnpm --filter web lint
```

**Commit boundary:** `feat(web): show agent coordination on Plans`

## Task 8: Document the Workflow and Run Final Verification

**Files:**

- Create `docs/guide/features/agent-coordination.md`
- Modify `docs/guide/features/plans.md`
- Modify `docs/guide/operations/integrations/mcp.md`
- Modify `CLAUDE.md`

**Produces:** operator and agent guidance, plus a verified implementation.

- [ ] Document the root/child workflow with concrete `join_board → claim_task → write_board_entry → update_board_task → ack_board` examples.
- [ ] State clearly that persistence is not delivery: agents must poll, and the host must wake/steer live agents.
- [ ] Document lease defaults, renewal guidance, cursor behavior, reconnect keys, idempotency keys, and local implicit-session assumptions.
- [ ] Update the project overview with the three new tables and the Rust-first module location.
- [ ] Regenerate SQLx offline metadata after every query has landed:

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
cargo sqlx prepare --workspace -- --tests
```

- [ ] Run final verification:

```bash
export DATABASE_URL="postgres://postgres@localhost:5432/fubbik_rs"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
SQLX_OFFLINE=true cargo check --workspace --all-targets
pnpm --filter @fubbik/mcp test
pnpm exec tsc -p packages/mcp/tsconfig.json --noEmit
pnpm --filter web test
pnpm --filter web check-types
pnpm --filter web lint
pnpm run fmt:check
```

- [ ] Compare final failures with Task 0 and report every remaining pre-existing failure explicitly.
- [ ] Review the staged diff using explicit pathspecs. Do not stage unrelated pre-existing changes and never use `git add -A`.

**Commit boundary:** `docs: document persistent agent coordination`

## Acceptance Scenarios

### Root delegates and later reconnects

1. Root joins Plan `P` with external key `thread-17/root`.
2. Child joins `P` with external key `thread-17/researcher` and root's run ID as parent.
3. Child claims task `T`, posts two progress entries, completes `T` with a handoff note, and disappears.
4. Root reconnects with `thread-17/root` and receives the same run ID.
5. Reading after its old cursor shows the progress and handoff, `T` is done, and no active claim remains.

### Two children race

1. Runs A and B submit claim requests for task `T` concurrently.
2. Exactly one receives the active claim.
3. The loser receives `409` containing the holder and lease expiry, without overwriting the row.
4. After expiry, the loser can claim `T` without cleanup work.

### Retry after an uncertain response

1. A child writes a handoff with `clientMutationId=m-42` but loses the response.
2. It repeats the write with the same ID.
3. Both responses identify the same entry and sequence; the journal contains one row.

### Tenant isolation

1. Alice and Bob each own a Plan and agent runs.
2. Bob supplies Alice's Plan, task, run, claim, and entry IDs to every endpoint.
3. Each request returns not-found and Alice's board is byte-for-byte unchanged.

## Follow-ups Deliberately Deferred

1. Bearer-token or scoped API-key authentication for remote MCP processes.
2. Host bridge that translates a durable direct entry into a live agent steer/wake operation.
3. MCP resource template such as `fubbik://plans/{planId}/board` and subscription notifications.
4. Human-authored board entries with an explicit `user | agent_run | system` author model.
5. Artifact entities, file uploads, and structured code-diff attachments.
6. Retention/compaction: summarize old journal spans while retaining decisions and handoffs.
7. Capability-based routing and automatic task matching.

## Exit Criteria

- A root and child can reconnect across process restarts without losing identity, messages, cursors, task state, or attribution.
- Concurrent claims have one winner, leases recover from crashed agents, and terminal transitions release claims atomically.
- Journal pagination is deterministic, retry-safe, and cursor acknowledgement is monotonic.
- Cross-user and cross-Plan isolation is enforced in SQL-backed repository paths and proven by tests.
- MCP exposes the full workflow without process-global state.
- The existing Plan UI shows agents, claims, and journal entries without duplicating Plan tasks.
- OpenAPI, generated web types, Drizzle schema, Rust migrations, and SQLx offline metadata agree.
- Focused tests, workspace verification, formatting, linting, and typechecking pass apart from explicitly recorded baseline failures.
