# MCP Review Workflow — AI Implementer / Human Reviewer

## Overview

Extend the Fubbik MCP server to support a structured workflow where AI implements and humans review. The AI records what context it used, what it assumed, and which requirements it addressed. Fubbik generates a review brief so the developer can review against intent rather than raw diffs. Feedback flows back as new knowledge.

## 1. Schema — Implementation Sessions

### `implementation_session`

| Column | Type | Notes |
|--------|------|-------|
| `id` | text PK | |
| `title` | text NOT NULL | Task description |
| `status` | text NOT NULL DEFAULT 'in_progress' | in_progress, completed, reviewed |
| `userId` | text FK → user (cascade) | |
| `codebaseId` | text FK → codebase (set null) | nullable |
| `prUrl` | text | nullable — linked PR or branch |
| `reviewBrief` | text | nullable — generated markdown |
| `createdAt` | timestamp DEFAULT now() | |
| `updatedAt` | timestamp DEFAULT now(), $onUpdate | auto-updates on changes |
| `completedAt` | timestamp | nullable |
| `reviewedAt` | timestamp | nullable |

Indexes: userId, codebaseId, status.

### `session_chunk_ref`

| Column | Type | Notes |
|--------|------|-------|
| `sessionId` | text FK → implementation_session (cascade) | |
| `chunkId` | text FK → chunk (cascade) | |
| `reason` | text NOT NULL | Why the AI referenced this chunk |

Primary key: (sessionId, chunkId).

### `session_assumption`

| Column | Type | Notes |
|--------|------|-------|
| `id` | text PK | |
| `sessionId` | text FK → implementation_session (cascade) | |
| `description` | text NOT NULL | What the AI had to guess |
| `resolved` | boolean DEFAULT false | |
| `resolution` | text | nullable — reviewer's resolution |

Index: sessionId.

### `session_requirement_ref`

| Column | Type | Notes |
|--------|------|-------|
| `sessionId` | text FK → implementation_session (cascade) | |
| `requirementId` | text FK → requirement (cascade) | |
| `stepsAddressed` | jsonb DEFAULT '[]' | Array of step indices addressed |

Primary key: (sessionId, requirementId).

## 2. MCP Tools

Extend the existing MCP server at `packages/mcp/`. Add 5 new tools alongside the existing 6.

### `begin_implementation`

Input: `{ title: string, codebaseId?: string }`

Behavior:
1. Creates an `implementation_session` record via `POST /api/sessions`
2. The API returns the session plus a context bundle:
   - Convention chunks with `appliesTo` patterns matching the codebase (or all conventions if no codebase)
   - Active requirements for the codebase (status != reviewed-out)
   - Architecture decision chunks (type = "document" with rationale)
3. Returns sessionId + context bundle to the AI

### `record_chunk_reference`

Input: `{ sessionId: string, chunkId: string, reason: string }`

Behavior: `POST /api/sessions/:id/chunk-refs`. Logs that the AI used a chunk. Reason is a short explanation (e.g., "followed this error handling convention").

### `record_assumption`

Input: `{ sessionId: string, description: string }`

Behavior: `POST /api/sessions/:id/assumptions`. Logs something the AI had to guess.

### `record_requirement_addressed`

Input: `{ sessionId: string, requirementId: string, stepsAddressed?: number[] }`

Behavior: `POST /api/sessions/:id/requirement-refs`. Logs which requirement (and optionally which steps) the AI implemented.

### `complete_implementation`

Input: `{ sessionId: string, prUrl?: string }`

Behavior: `PATCH /api/sessions/:id/complete`. Marks session complete, triggers review brief generation server-side. Returns the generated brief.

## 3. Review Brief Generation

Generated server-side when `complete_implementation` is called. Stored as markdown in `implementation_session.reviewBrief`.

### Sections

**Requirements Addressed:**
- For each referenced requirement: title, current status, steps addressed vs total steps
- Flag partial coverage (e.g., "2/3 steps addressed")
- Flag requirements in the codebase NOT addressed by this session (potential misses)

**Conventions Applied:**
- For each referenced convention/note chunk: title, the AI's reason for referencing it
- Flag convention chunks matching the codebase that were NOT referenced (potential violations)

**Assumptions Made:**
- List each assumption with description
- These become interactive checkboxes in the UI

**Knowledge Gaps:**
- Derived from assumptions (areas where no chunk existed)
- Aggregated with frequency across sessions for the gap tracker

**Summary Stats:**
- Requirements addressed count / total for codebase
- Chunks referenced count
- Assumptions made count
- Session duration (createdAt → completedAt)

### Generation Logic

The service function `generateReviewBrief(sessionId)`:
1. Fetches session with all refs and assumptions
2. Fetches all requirements for the codebase (to compare addressed vs total)
3. Fetches all convention chunks for the codebase (to compare referenced vs available)
4. Builds markdown string from template
5. Stores in `implementation_session.reviewBrief`

## 4. API Endpoints

New route group under `/api/sessions`. Follow existing patterns (Elysia + Effect + requireSession).

**Auth note:** The MCP server runs locally and currently makes unauthenticated API calls. The existing API falls back to a `DEV_SESSION` in dev mode (hardcoded `dev-user`). The session endpoints must work under this same fallback. The MCP tools pass no auth — they rely on the dev-mode session. This is acceptable for V1 (local-only).

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/sessions` | Create session. Body: `{ title, codebaseId? }`. Returns session + context bundle. 201. |
| GET | `/api/sessions` | List sessions. Query: `{ status?, codebaseId?, limit?, offset? }`. |
| GET | `/api/sessions/:id` | Get session detail with chunk refs, assumptions, requirement refs, review brief. |
| PATCH | `/api/sessions/:id/complete` | Mark complete, generate review brief. Body: `{ prUrl? }`. |
| POST | `/api/sessions/:id/chunk-refs` | Add chunk reference. Body: `{ chunkId, reason }`. 201. |
| POST | `/api/sessions/:id/assumptions` | Add assumption. Body: `{ description }`. 201. |
| PATCH | `/api/sessions/:id/assumptions/:assumptionId` | Resolve assumption. Body: `{ resolved, resolution? }`. |
| POST | `/api/sessions/:id/requirement-refs` | Add requirement ref. Body: `{ requirementId, stepsAddressed? }`. 201. |
| PATCH | `/api/sessions/:id/review` | Mark reviewed, optionally update requirement statuses. Body: `{ requirementStatuses?: Array<{ requirementId, status }> }`. |

### Context Bundle (returned by POST /api/sessions)

```typescript
{
  session: { id, title, status, ... },
  context: {
    conventions: Array<{ id, title, content, appliesTo? }>,
    requirements: Array<{ id, title, steps, status, priority }>,
    architectureDecisions: Array<{ id, title, content, rationale, alternatives, consequences }>
  }
}
```

The context bundle fetches:
- Conventions: chunks that have convention-related tags (e.g., tags containing "convention", "standard", "rule") OR have a non-null `rationale` field. This matches the existing `get_conventions` MCP tool's approach. Filtered by codebase if provided.
- Requirements: all requirements for the codebase (or all if no codebase specified). No status filtering — include all active requirements.
- Architecture decisions: chunks where `type = 'document'` and `rationale IS NOT NULL` and tagged with "architecture" or "decision" (to avoid over-matching all documents with rationale).

## 5. Web UI

### Reviews List Page (`/reviews`)

- Card per session: title, status badge (in_progress=blue, completed=amber, reviewed=green), codebase name, creation date
- Stats on each card: N requirements addressed, N assumptions (N unresolved)
- Filter by status, codebase
- Click navigates to detail

### Review Detail Page (`/reviews/:sessionId`)

Renders the review brief with interactive elements:

**Requirements Addressed section:**
- Each requirement is a link to its detail page
- Shows status badge + "N/M steps addressed"
- Unreferenced requirements for the codebase shown in a collapsed "Not addressed" section

**Conventions Applied section:**
- Each convention is a link to its chunk detail page
- Shows the AI's reason
- Unreferenced conventions shown in a collapsed "Not checked" section

**Assumptions section:**
- Each assumption has:
  - Description text
  - "Resolve" button → expands inline form with resolution text input
  - "Create chunk" button → navigates to chunk creation with description pre-filled as content
  - Status indicator (unresolved/resolved)

**Knowledge Gaps section:**
- Aggregated from assumptions
- "Create chunk" shortcut per gap

**Actions:**
- "Mark as Reviewed" button: sets status to reviewed, shows a form to set requirement statuses (passing/failing for each addressed requirement)
- Link to PR if prUrl is set

### Nav Integration

Add "Reviews" to the primary nav items (between Requirements and Docs in the nav config at `apps/web/src/features/nav/mobile-nav.tsx` and `apps/web/src/routes/__root.tsx`).

### Requirement Detail Integration

On the requirement detail page (`requirements_.$requirementId.tsx`), add a small "Implementation Sessions" section listing sessions that referenced this requirement. Each links to the review page.

### Knowledge Health Integration

On the existing `/knowledge-health` page, add a "Knowledge Gaps from AI Sessions" section:
- Aggregates unresolved assumptions across all sessions
- Groups by similarity (exact text match for V1)
- Shows frequency count ("AI assumed this 3 times across 3 sessions")
- Each gap links to the sessions where it appeared
- "Create chunk" shortcut per gap

## 6. Requirement Status Auto-Updates

When the reviewer marks a session as reviewed (`PATCH /api/sessions/:id/review`), they can include requirement status updates:

```json
{
  "requirementStatuses": [
    { "requirementId": "req-04", "status": "passing" },
    { "requirementId": "req-05", "status": "failing" }
  ]
}
```

The service:
1. Updates the session status to "reviewed" with `reviewedAt = now()`
2. For each requirement status update, calls `updateRequirementStatus(id, userId, status)`
3. All in a single logical operation (if any requirement update fails, the session is still marked reviewed — partial updates are acceptable)

## Files to Create/Modify

### New files
- `packages/db/src/schema/implementation-session.ts` — session + refs + assumptions tables
- `packages/db/src/repository/implementation-session.ts` — session CRUD, ref management, assumption resolution
- `packages/api/src/sessions/routes.ts` — API endpoints
- `packages/api/src/sessions/service.ts` — business logic + review brief generation
- `packages/api/src/sessions/brief-generator.ts` — review brief markdown generation
- `packages/mcp/src/session-tools.ts` — new MCP tool definitions
- `apps/web/src/routes/reviews.tsx` — reviews list page
- `apps/web/src/routes/reviews_.$sessionId.tsx` — review detail page
- `apps/web/src/features/reviews/session-card.tsx` — card component for list
- `apps/web/src/features/reviews/assumption-resolver.tsx` — inline assumption resolution UI

### Modified files
- `packages/db/src/schema/index.ts` — add session schema export
- `packages/db/src/repository/index.ts` — add session repository export
- `packages/api/src/index.ts` — register session routes
- `packages/mcp/src/index.ts` — register new tools
- `packages/mcp/src/index.ts` — import and call `registerSessionTools(server)` from session-tools.ts
- `apps/web/src/routes/__root.tsx` — add Reviews to nav
- `apps/web/src/features/nav/mobile-nav.tsx` — add Reviews to mobile nav
- `apps/web/src/routes/requirements_.$requirementId.tsx` — add sessions section

## Out of Scope

- API key authentication (local-only for V1)
- Automatic file-to-chunk matching (no static analysis of which files the AI touched)
- Diff-level annotation (linking specific code changes to specific chunks)
- Multi-user review assignment
- Assumption similarity clustering beyond exact text match
