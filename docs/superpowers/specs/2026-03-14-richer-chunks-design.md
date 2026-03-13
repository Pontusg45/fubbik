# Richer Chunks — Design Spec

## Overview

Enrich the chunk data model with four capabilities:

1. **Structured metadata (`appliesTo`)** — glob patterns with optional descriptions linking chunks to file areas
2. **File references** — explicit bidirectional links between chunks and files/symbols
3. **"Why" fields** — optional rationale, alternatives, and consequences on any chunk
4. **Templates** — built-in + user-created chunk templates for consistent knowledge capture

These are the foundation for AI-optimized features (system prompt export, CLAUDE.md generation, onboarding mode) planned in future specs.

## Data Model

### New `chunk_applies_to` table

| Column | Type | Notes |
|--------|------|-------|
| `id` | text (PK) | UUID as text |
| `chunkId` | text (FK → chunk) | ON DELETE CASCADE |
| `pattern` | text | Glob pattern, e.g. `src/auth/**` |
| `note` | text (nullable) | Optional description, e.g. "auth module" |

**Indexes:** `chunkId`

**Authorization:** No `userId` column — ownership verified in the service layer by checking `chunk.userId` matches the session user before any mutation or read. Same pattern as `chunkTag`.

### New `chunk_file_ref` table

| Column | Type | Notes |
|--------|------|-------|
| `id` | text (PK) | UUID as text |
| `chunkId` | text (FK → chunk) | ON DELETE CASCADE |
| `path` | text | File or directory path, e.g. `src/auth/session.ts` |
| `anchor` | text (nullable) | Symbol name, e.g. `SessionManager` (not line numbers — resilient to edits) |
| `relation` | text | One of: `documents`, `configures`, `tests`, `implements` |

**Indexes:** `chunkId`, `path` (for reverse lookup: "which chunks reference this file?")

**Authorization:** No `userId` column — ownership verified in the service layer by checking `chunk.userId` matches the session user before any mutation. The reverse lookup endpoint (`/file-refs/lookup`) must join through `chunk` to filter by `userId` to prevent cross-user data leaks.

### New columns on `chunk` table

| Column | Type | Notes |
|--------|------|-------|
| `rationale` | text (nullable) | Why this decision/convention exists |
| `alternatives` | jsonb (nullable) | `string[]` — other options considered |
| `consequences` | text (nullable) | Impact and trade-offs |

These are optional "why" fields. Any chunk type can have them — they're not restricted to a specific type.

### New `chunk_template` table

| Column | Type | Notes |
|--------|------|-------|
| `id` | text (PK) | UUID as text |
| `name` | text | e.g. "Convention", "Architecture Decision" |
| `description` | text (nullable) | What this template is for |
| `type` | text | Default chunk type (note, document, etc.) |
| `content` | text | Template body with placeholder text |
| `isBuiltIn` | boolean | true for shipped defaults, false for user-created |
| `userId` | text (FK → user, nullable) | null for built-in, set for user-created |
| `createdAt` | timestamp | |

**Constraints:** Two partial unique indexes to handle the NULL userId correctly:
- `uniqueIndex("template_user_name_idx").on(userId, name).where(isNotNull(userId))` — prevents duplicate names per user
- `uniqueIndex("template_builtin_name_idx").on(name).where(isNull(userId))` — prevents duplicate built-in template names

**Seeding:** Built-in templates are seeded via a SQL migration file in `run-sql-migrations.ts` using `INSERT ... ON CONFLICT (name) WHERE user_id IS NULL DO UPDATE SET content = EXCLUDED.content, description = EXCLUDED.description`. This is idempotent — runs on every deploy, updates content if templates are revised, never duplicates.

## Built-in Templates

Shipped with fubbik, read-only, seeded into the database:

**Convention**
```
## Rule

[What is the convention?]

## Rationale

[Why does this convention exist?]

## Examples

[Code examples showing correct usage]

## Exceptions

[When is it OK to break this rule?]
```

**Architecture Decision**
```
## Context

[What is the situation that requires a decision?]

## Decision

[What was decided?]

## Alternatives Considered

[What other options were evaluated?]

## Consequences

[What are the trade-offs and impacts?]
```

**Runbook**
```
## When to Use

[What situation triggers this runbook?]

## Steps

1. [Step 1]
2. [Step 2]

## Rollback

[How to undo if something goes wrong]

## Escalation

[Who to contact if this doesn't resolve the issue]
```

**API Endpoint**
```
## Endpoint

[METHOD /path]

## Request

[Headers, body, query params]

## Response

[Success and error responses]

## Authentication

[Auth requirements]

## Errors

[Error codes and their meaning]
```

## API Changes

### New endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/chunks/:id/applies-to` | List applies-to patterns for a chunk |
| `PUT` | `/chunks/:id/applies-to` | Replace all applies-to patterns |
| `GET` | `/chunks/:id/file-refs` | List file references for a chunk |
| `PUT` | `/chunks/:id/file-refs` | Replace all file references |
| `GET` | `/file-refs/lookup?path=<path>` | Reverse lookup: find chunks referencing a file |
| `GET` | `/templates` | List all templates (built-in + user's custom) |
| `POST` | `/templates` | Create custom template |
| `PATCH` | `/templates/:id` | Update custom template |
| `DELETE` | `/templates/:id` | Delete custom template (user-created only) |

### Modified endpoints

- `GET /chunks/:id` — response includes `appliesTo`, `fileReferences`, `rationale`, `alternatives`, `consequences`. These are fetched in the same `Effect.all` fan-out in `getChunkDetail` (alongside connections and codebases) to avoid extra round trips.
- `POST /chunks` — accepts optional `rationale`, `alternatives`, `consequences`, `appliesTo`, `fileReferences`, `templateId`. Template pre-filling is a **client-side concern**: the client fetches the template via `GET /templates`, then submits the pre-filled content as part of the normal `POST /chunks` body. No server-side template resolution.
- `PATCH /chunks/:id` — same optional fields for `rationale`, `alternatives`, `consequences`

### PUT semantics

`PUT /chunks/:id/applies-to` and `PUT /chunks/:id/file-refs` use the replace-all pattern (same as `setChunkTags`): send the full desired list, server deletes existing rows and inserts the new ones.

**Request body for applies-to:**
```json
[
  { "pattern": "src/auth/**", "note": "auth module" },
  { "pattern": "src/middleware/*" }
]
```

**Request body for file-refs:**
```json
[
  { "path": "src/auth/session.ts", "anchor": "SessionManager", "relation": "documents" },
  { "path": "src/auth/", "relation": "configures" }
]
```

### Backend pattern

Follows existing Repository → Service → Route pattern:

- New repositories: `chunk-applies-to.ts`, `chunk-file-ref.ts`, `chunk-template.ts` in `packages/db/src/repository/`
- New schemas: `applies-to.ts`, `file-ref.ts`, `template.ts` in `packages/db/src/schema/`
- New services and routes: `applies-to/`, `file-refs/`, `templates/` in `packages/api/src/`
- Chunk service/repository extended for the new JSONB columns
- Built-in templates seeded via a SQL migration in `run-sql-migrations.ts` (idempotent upsert)

## Web UI

### Chunk create/edit form

- **Template selector** — dropdown at the top of create form. Selecting a template pre-fills content and type. Shows built-in + user templates.
- **"Applies To" section** — repeatable field group: pattern input + optional note input. "Add pattern" button. Show below tags.
- **"File References" section** — repeatable field group: path input + optional anchor input + relation dropdown (`documents`, `configures`, `tests`, `implements`). "Add reference" button.
- **"Decision Context" section** — collapsible, collapsed by default. Expand via "Add decision context" link. Contains: rationale textarea, alternatives (repeatable text inputs), consequences textarea.

### Chunk detail page (`/chunks/:id`)

- **Applies-to patterns** — list of code-styled monospace badges, each showing the pattern and optional note
- **File references** — list with monospace path, anchor badge (if present), and relation label
- **Decision context** — distinct section (if any fields are present): rationale, alternatives as a list, consequences. Styled differently (e.g., slight background tint) to visually separate from the main content.

### Templates page (`/templates`)

New route. Nav link between "Health" and "Codebases".

- List all templates: built-in (with a "Built-in" badge, read-only) and user-created (editable, deletable)
- Create form: name, description, type dropdown, content textarea
- Edit form: same fields
- Delete: confirmation dialog, only for user-created

### Built-in templates display

Built-in templates are visually distinct (badge, no edit/delete actions). Users can "Duplicate" a built-in template to create a customizable copy. Duplication is client-side: fetch the template content via `GET /templates`, then `POST /templates` with the content and a new name.

## Future Considerations (Out of Scope)

- AI-powered template suggestion based on content
- Auto-detecting file references from chunk content (parsing file paths mentioned in text)
- Validating applies-to patterns against the actual repository file tree
- Template variables / placeholder syntax beyond plain text
