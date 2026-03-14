# Requirements System — Design Spec

## Overview

A structured requirements system for fubbik that enables writing verifiable Given/When/Then specifications. Requirements are a separate entity (not chunks) that can be linked to chunks, statically validated, cross-referenced against the codebase, and exported to multiple test formats (Gherkin, Vitest, markdown checklists).

This is sub-project 1 of the requirements feature. Future sub-projects: CI integration for automated status updates, requirement coverage reporting.

## Data Model

### New `requirement` table

| Column | Type | Notes |
|--------|------|-------|
| `id` | text (PK) | UUID as text |
| `title` | text | Short description, e.g. "Authenticated user can view dashboard" |
| `description` | text (nullable) | Longer context / acceptance criteria prose |
| `steps` | jsonb | Structured Given/When/Then steps (see below) |
| `status` | text | `untested`, `passing`, `failing` — default `untested` |
| `priority` | text (nullable) | `must`, `should`, `could`, `wont` (MoSCoW) |
| `codebaseId` | text (FK → codebase, nullable) | ON DELETE SET NULL |
| `userId` | text (FK → user) | ON DELETE CASCADE |
| `createdAt` | timestamp | |
| `updatedAt` | timestamp | Auto-updated via Drizzle `$onUpdate(() => new Date())` |

**Indexes:** `userId`, `codebaseId`, `status`

**DB constraint:** `CHECK (jsonb_array_length(steps) > 0)` — enforces at least one step at the database level as a safety net.

### New `requirement_chunk` join table

| Column | Type | Notes |
|--------|------|-------|
| `requirementId` | text (FK → requirement) | ON DELETE CASCADE |
| `chunkId` | text (FK → chunk) | ON DELETE CASCADE |

**Composite PK:** `(requirementId, chunkId)`

Links requirements to related chunks (e.g., "this requirement documents behavior described in the 'Auth Flow' chunk").

**Authorization:** The service layer must verify all supplied `chunkId` values belong to the authenticated user before inserting into the join table, to prevent cross-user data leaks.

### Steps JSONB structure

```typescript
interface RequirementStep {
    keyword: "given" | "when" | "then" | "and" | "but";
    text: string;
    params?: Record<string, string>; // optional typed parameters
}
```

**Example:**

```json
[
    { "keyword": "given", "text": "a user is logged in" },
    { "keyword": "and", "text": "they have 3 chunks in their knowledge base" },
    { "keyword": "when", "text": "they visit /dashboard" },
    { "keyword": "then", "text": "they see a stats card showing '3 chunks'" },
    { "keyword": "and", "text": "they see their recent chunks" }
]
```

### Step validation rules (enforced on write)

1. Must have at least one step
2. First step must be `given`
3. Must contain at least one `when` and one `then`
4. `and`/`but` inherit the semantic role of the preceding non-and/but step
5. `when` must come after all `given` steps (and their `and`/`but` continuations)
6. `then` must come after all `when` steps (and their `and`/`but` continuations)
7. Valid sequence: `given` block → `when` block → `then` block, where each block is one keyword step followed by zero or more `and`/`but` steps

Validation returns specific error messages with step index, e.g.: `{ step: 0, error: "First step must be 'given'" }`.

## Cross-Referencing (Static Checks)

When a requirement is saved, an optional validation pass checks step text against known entities in fubbik's database.

### What it checks

- **File paths** — regex extracts paths like `src/auth/session.ts` from step text, checked via exact match against `chunk_file_ref.path`. Does NOT glob-match against `chunk_applies_to` patterns (too ambiguous).
- **API endpoints** — regex extracts HTTP method + path patterns like `GET /api/chunks`, matched as prefix against known route patterns
- **Chunk references** — quoted strings like `'Auth Flow'` matched case-insensitively against existing chunk titles

### How it works

Simple regex extraction on each step's text. Matched against the database — not the filesystem. Fubbik checks what it knows about, not what's on disk.

Returns **warnings, not errors** — a missing reference doesn't block saving. This keeps the system usable even when the knowledge base is incomplete.

### Response shape

```json
{
    "requirement": { "..." },
    "warnings": [
        { "step": 2, "type": "file_not_found", "reference": "src/auth/session.ts" },
        { "step": 3, "type": "endpoint_not_found", "reference": "GET /api/users" }
    ]
}
```

Warning types: `file_not_found`, `endpoint_not_found`, `chunk_not_found`.

## Export Adapters

Three output formats generated from the `steps` JSONB:

### Gherkin (`.feature`)

```gherkin
Feature: Authenticated user can view dashboard

  Scenario: Authenticated user can view dashboard
    Given a user is logged in
    And they have 3 chunks in their knowledge base
    When they visit /dashboard
    Then they see a stats card showing '3 chunks'
    And they see their recent chunks
```

### Vitest (`.test.ts`)

```typescript
import { describe, it } from "vitest";

describe("Authenticated user can view dashboard", () => {
    it("should show dashboard with chunks", () => {
        // Given a user is logged in
        // And they have 3 chunks in their knowledge base
        // When they visit /dashboard
        // Then they see a stats card showing '3 chunks'
        // And they see their recent chunks
        throw new Error("Not implemented");
    });
});
```

### Markdown checklist

```markdown
## Authenticated user can view dashboard

- [ ] Given a user is logged in
- [ ] And they have 3 chunks in their knowledge base
- [ ] When they visit /dashboard
- [ ] Then they see a stats card showing '3 chunks'
- [ ] And they see their recent chunks
```

### Export notes

- Step `params` are interpolated into the text if present (e.g., `"a user with email {email}"` + `{ email: "test@example.com" }` → `"a user with email test@example.com"`)
- Gherkin uses proper indentation and keyword capitalization
- Vitest generates a `describe` block per requirement with a single `it` containing all steps as comments and a `throw new Error("Not implemented")` placeholder
- Markdown uses the requirement title as heading and steps as a checklist

## API

### New endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/requirements` | List (filter by `codebaseId`, `status`, `priority`) |
| `POST` | `/requirements` | Create (validates steps, returns warnings) |
| `GET` | `/requirements/:id` | Detail with linked chunks |
| `PATCH` | `/requirements/:id` | Update (re-validates, returns warnings) |
| `DELETE` | `/requirements/:id` | Delete |
| `PATCH` | `/requirements/:id/status` | Update status only (`passing`/`failing`/`untested`) |
| `PUT` | `/requirements/:id/chunks` | Set linked chunks (replace-all) |
| `GET` | `/requirements/:id/export` | Export single (`?format=gherkin\|vitest\|markdown`) |
| `GET` | `/requirements/export` | Bulk export (`?format=...&codebaseId=`) |
| `GET` | `/requirements/stats` | Summary counts by status |

### Route declaration order

In `routes.ts`, declare in this exact order:
1. `GET /requirements/stats`
2. `GET /requirements/export`
3. `GET /requirements` (list)
4. `POST /requirements`
5. `GET /requirements/:id`
6. `PATCH /requirements/:id`
7. `DELETE /requirements/:id`
8. `PATCH /requirements/:id/status`
9. `PUT /requirements/:id/chunks`
10. `GET /requirements/:id/export`

Static paths (`/stats`, `/export`) must come before `/:id` or Elysia will match them as dynamic IDs.

### PATCH semantics

PATCH fetches the existing record first (following the `updateChunk` pattern). Step validation only runs if `steps` is present in the body. Cross-reference warnings are only returned when `steps` is included. If only `title` or `description` is updated, no validation or cross-referencing occurs.

### Error handling

Step validation failures use a new `StepValidationError` tagged error in `packages/api/src/errors.ts`:

```typescript
export class StepValidationError extends Data.TaggedError("StepValidationError")<{
    errors: Array<{ step: number; error: string }>;
}> {}
```

Mapped to HTTP 400 in the global error handler, returning `{ message: "Invalid steps", errors: [...] }`.

### Backend pattern

Follows Repository → Service → Route:

- `packages/db/src/schema/requirement.ts` — requirement + requirement_chunk tables
- `packages/db/src/repository/requirement.ts` — CRUD + chunk linking
- `packages/api/src/requirements/validator.ts` — step sequence validation
- `packages/api/src/requirements/cross-ref.ts` — cross-reference checker
- `packages/api/src/requirements/export.ts` — Gherkin/Vitest/markdown adapters
- `packages/api/src/requirements/service.ts` — composes validation + cross-ref + CRUD
- `packages/api/src/requirements/routes.ts` — HTTP endpoints

## CLI

- `fubbik requirements list` — list requirements (`--status`, `--codebase`, `--priority`)
- `fubbik requirements add <title> --step "given: ..." --step "when: ..." --step "then: ..."` — create via flags (non-interactive, consistent with existing CLI patterns). Steps use `keyword: text` format.
- `fubbik requirements status <id> passing|failing|untested` — update status
- `fubbik requirements export --format gherkin|vitest|markdown` — export to stdout (all requirements for detected codebase, or `--codebase` override)
- `fubbik requirements verify` — re-run cross-reference checks, report warnings

## Web UI

### Requirements list page (`/requirements`)

- Stats summary bar at top: total, passing (green), failing (red), untested (gray)
- Filterable by status, priority, codebase
- Each row: title, status badge, priority badge, linked chunk count, created date
- Quick actions: change status, delete

### Create/edit page (`/requirements/new`, `/requirements/:id/edit`)

- Title input
- Description textarea
- Priority selector (must/should/could/won't)
- Codebase selector
- **Step builder:**
  - Ordered list of steps
  - Each step: keyword dropdown (given/when/then/and/but) + text input + optional params
  - "Add step" button
  - Drag to reorder
  - Remove button per step
  - Real-time validation — highlights invalid sequences
- Linked chunks: multi-select search
- On save: shows cross-reference warnings if any

### Detail page (`/requirements/:id`)

- Title, description, priority badge, status badge
- Steps displayed as a formatted Given/When/Then block
- Cross-reference warnings (if any)
- Linked chunks as clickable links
- Export buttons: Gherkin, Vitest, Markdown (download or copy to clipboard)
- Status toggle: passing/failing/untested

### Nav

Add "Requirements" link to navigation (desktop + mobile).

## Future Considerations (Out of Scope)

- CI integration: automated status updates from test runs
- Coverage reporting: percentage of requirements with passing tests
- Vitest export with multiple `it` blocks for multi-scenario requirements
- Requirement dependencies: "requirement B depends on requirement A"
- Step libraries: reusable step definitions shared across requirements
- Parameterized scenarios: Scenario Outline with Examples tables (Gherkin feature)
- Watch mode: `fubbik requirements watch` that re-checks cross-references when files change
