# Chunk Proposal Review Workflow

## Problem

When an AI agent modifies an existing chunk via the MCP `update_chunk` tool, the change is applied immediately — there's no human review step. The version history captures the before-state, but only after the fact. For a knowledge base where accuracy matters, AI edits should be staged for human review before going live.

AI-created chunks already start as `draft` (via the existing `reviewStatus` field), but edits to existing chunks bypass this entirely.

## Goal

Add a proposal system where AI agents suggest changes to existing chunks. Proposals are saved as pending drafts. Humans review proposals in a dedicated queue or inline on the chunk detail page, then approve (applies the change) or reject (discards it). Multiple proposals can be pending per chunk simultaneously.

---

## 1. Data Model

One new table. No changes to existing tables.

### `chunk_proposal`

| column | type | notes |
|---|---|---|
| `id` | `text` pk | `$defaultFn(() => crypto.randomUUID())` |
| `chunkId` | `text` fk → `chunk.id` cascade delete | the chunk being changed |
| `changes` | `jsonb` not null | snapshot of proposed field values — only changed fields present |
| `reason` | `text` nullable | optional explanation of why the AI proposed this change |
| `status` | `text` not null default `"pending"` | `pending \| approved \| rejected \| superseded` |
| `proposedBy` | `text` not null | user ID or `"ai"` |
| `reviewedBy` | `text` nullable fk → `user.id` | who approved/rejected |
| `reviewedAt` | `timestamp` nullable | when approved/rejected |
| `reviewNote` | `text` nullable | reviewer's comment (optional) |
| `createdAt` | `timestamp` not null defaultNow | |

Indexes: `(chunkId)`, `(status)`, `(chunkId, status)`.

### `changes` JSONB shape

Any subset of chunk fields. Only fields the AI wants to change are included:

```typescript
interface ProposedChanges {
    title?: string;
    content?: string;
    type?: string;
    tags?: string[];
    rationale?: string;
    alternatives?: string[];
    consequences?: string;
    scope?: Record<string, string>;
}
```

### Stacking

Multiple `pending` proposals can exist per chunk. They are ordered by `createdAt` and reviewed independently.

---

## 2. API Surface

### Proposal CRUD

| method | path | body / query | notes |
|---|---|---|---|
| `POST` | `/api/chunks/:id/proposals` | `{ changes: ProposedChanges, reason?: string }` | Creates a pending proposal. `proposedBy` from session user (or `"ai"` via MCP). |
| `GET` | `/api/chunks/:id/proposals` | `status?` | List proposals for a chunk. Returns newest first. |
| `GET` | `/api/proposals` | `status?` (default: `pending`), `chunkId?`, `limit?`, `offset?` | Global review queue. Returns proposals with chunk title + type for display context. |
| `GET` | `/api/proposals/:proposalId` | — | Single proposal with full chunk snapshot for diff rendering. |
| `POST` | `/api/proposals/:proposalId/approve` | `{ note?: string }` | Applies `changes` to the chunk via existing `updateChunk` flow. Sets `status: approved`, `reviewedBy`, `reviewedAt`. |
| `POST` | `/api/proposals/:proposalId/reject` | `{ note?: string }` | Sets `status: rejected`. Chunk untouched. |
| `POST` | `/api/proposals/bulk` | `{ actions: Array<{ proposalId: string, action: "approve" \| "reject", note?: string }> }` | Batch approve/reject. Processes in order. |
| `GET` | `/api/proposals/count` | — | Returns `{ pending: number }`. Powers the nav badge. |

### MCP tool (new)

New tool: `propose_chunk_update`

Parameters:
- `chunkId: string` (required)
- `changes: ProposedChanges` (required) — same shape as the JSONB
- `reason?: string` (optional)

Calls `POST /api/chunks/:id/proposals` internally. Returns the created proposal.

The existing `update_chunk` tool is unchanged and continues to apply direct edits for human-initiated or trusted flows.

---

## 3. Approve / Reject Mechanics

### Approve

1. `POST /api/proposals/:proposalId/approve` is called
2. Service reads the proposal's `changes` JSONB
3. Calls the existing `updateChunk(chunkId, userId, changes)` — which:
   - Creates a `chunk_version` row (captures before-state)
   - Updates the live chunk fields
   - Triggers async re-enrichment if title or content changed
4. If `changes.tags` is present, calls `setChunkTags(chunkId, changes.tags)` to replace the tag list
5. Sets proposal `status: approved`, `reviewedBy: userId`, `reviewedAt: now()`
6. Returns the updated chunk

Approving a proposal is identical to a human making the same edit manually. No new update path.

### Reject

1. `POST /api/proposals/:proposalId/reject` is called
2. Sets `status: rejected`, `reviewedBy`, `reviewedAt`, `reviewNote`
3. Chunk is untouched
4. Returns `{ ok: true }`

### Conflict handling

If the live chunk was edited between proposal creation and approval, the proposal still applies — it overwrites the fields it touches. The reviewer sees the current live state and the proposed state at review time, so they can judge whether the proposal is still relevant. If the chunk has diverged significantly, the reviewer rejects and the AI re-proposes.

No automatic conflict detection or merge.

---

## 4. Review UI

### 4a: Dedicated review queue page — `/review`

- Header: "Review Queue" with pending count
- Filter bar: status toggle (`pending` / `approved` / `rejected` / `all`), optional chunk search
- List of proposal cards, each showing:
  - Chunk title + type badge
  - Proposal age ("2h ago")
  - Reason (if provided, truncated to one line)
  - Changed fields summary as small pills (e.g., `title`, `content`, `tags`)
  - Approve / Reject buttons directly on the card (quick action)
  - Click card → expands to show field-by-field diff inline
- Field-by-field diff format: field name → current value → proposed value, with changed text highlighted. For `content` diffs: show truncated inline diff (first 5 changed lines), expandable to full.
- Bulk actions: "Approve all" / "Reject all" for the visible set (with confirmation dialog)
- Empty state: "No proposals waiting for review"

### 4b: Inline on chunk detail page — `/chunks/:id`

When a chunk has pending proposals, a collapsible section appears below the existing content:

- Section header: "Pending proposals (N)" with amber indicator
- Each proposal as a compact card:
  - Reason (if any)
  - Field-by-field diff (field name → current → proposed, changed text highlighted)
  - For `content`: truncated diff, expandable
  - Approve / Reject buttons
  - Timestamp + "proposed by AI"
- Proposals ordered oldest-first (review in chronological order)

### 4c: Nav badge

- A count badge on the "Chunks" primary nav link (or in the Manage dropdown) showing the number of pending proposals
- Uses `GET /api/proposals/count`
- Polls every 60 seconds (or piggybacks on an existing polling mechanism if one exists)

---

## 5. Files Changed

### New files

| Path | Responsibility |
|---|---|
| `packages/db/src/schema/chunk-proposal.ts` | Schema definition for `chunk_proposal` table |
| `packages/db/src/repository/chunk-proposal.ts` | Effect-based CRUD for proposals |
| `packages/api/src/proposals/service.ts` | Proposal business logic (create, approve with updateChunk, reject, list, count) |
| `packages/api/src/proposals/routes.ts` | Elysia route definitions |
| `apps/web/src/routes/review.tsx` | Review queue page |
| `apps/web/src/features/proposals/proposal-card.tsx` | Reusable proposal card with diff + approve/reject |
| `apps/web/src/features/proposals/proposal-diff.tsx` | Field-by-field diff renderer |
| `apps/web/src/features/proposals/chunk-proposals-section.tsx` | Inline section for chunk detail page |

### Modified

| Path | Change |
|---|---|
| `packages/db/src/schema/index.ts` | Add `chunk-proposal` export |
| `packages/db/src/repository/index.ts` | Add `chunk-proposal` export |
| `packages/api/src/index.ts` | Mount proposal routes |
| `packages/mcp/src/tools.ts` (or new `proposal-tools.ts`) | Add `propose_chunk_update` tool |
| `packages/mcp/src/index.ts` | Register proposal tools |
| `apps/web/src/routes/__root.tsx` | Add nav badge for pending proposals |
| `apps/web/src/routes/chunks.$chunkId.tsx` (or relevant detail component) | Mount `ChunkProposalsSection` inline |

### Unchanged

- `PATCH /api/chunks/:id` — direct edit flow is untouched
- `update_chunk` MCP tool — unchanged, still does direct edits
- `chunk` table — no schema changes
- `chunk_version` table — untouched (versions are created by the approve flow via `updateChunk`)

---

## 6. Out of Scope

- **Partial approve** — can't approve some fields and reject others from one proposal. Approve or reject the whole snapshot.
- **Edit-before-approve** — reviewer can't modify proposed values before approving. Approve as-is or reject.
- **Auto-approve rules** — no rules like "auto-approve if only tags changed." All proposals require human action.
- **Proposal expiry** — proposals stay `pending` indefinitely until reviewed.
- **Push notifications** — no email/push on proposal creation. Discovery via nav badge + review queue.
- **Proposal comments/discussion** — no thread. Just the single `reviewNote` on approve/reject.
- **Chunk creation proposals** — this spec covers edits to existing chunks only. AI-created chunks already start as `draft` via the existing `reviewStatus` field.
- **Connection/applies-to/file-ref proposals** — `changes` JSONB covers chunk fields only. Proposing structural changes (connections, file refs) is a separate concern.

---

## Success Criteria

- `chunk_proposal` table exists with the defined schema
- `POST /api/chunks/:id/proposals` creates a pending proposal with JSONB changes
- `GET /api/proposals` returns the global queue filtered by status
- `POST /api/proposals/:id/approve` applies changes via `updateChunk` (creates version, triggers enrichment)
- `POST /api/proposals/:id/reject` marks rejected without touching the chunk
- `POST /api/proposals/bulk` batch processes approve/reject actions
- `GET /api/proposals/count` returns `{ pending: N }` for the nav badge
- MCP `propose_chunk_update` tool creates proposals (separate from `update_chunk`)
- `/review` page shows pending proposals with field-by-field diffs and approve/reject buttons
- Chunk detail page shows an inline "Pending proposals" section when proposals exist
- Nav badge shows pending count, polls every 60s
- Multiple pending proposals per chunk are supported and independently reviewable
