# Behavioral Specification Matrix

**Date**: 2026-05-14 **Status**: Draft

## Problem

Fubbik's requirements are BDD scenarios (Given/When/Then) — concrete and detailed, but impossible to zoom out from. ADRs are narrative
chunks with rationale fields. Neither gives a scannable, structured view of what the system should do. This creates three problems:

1. **Gaps in coverage**: no way to see which behaviors are specified and which are missing.
2. **Communicating behavior**: no single view where a human or AI can understand "here is exactly what this system does" without reading all
   the docs.
3. **Verifying correctness**: no way to check whether the implementation matches what was decided.

## Solution

A **Behavioral Specification Matrix** — a first-class spec layer above BDD requirements. The matrix is the top-level declaration of what the
system should do. BDD requirements are the concrete scenarios that prove individual cells.

### Hierarchy

```
Behavioral Matrix (abstract — "what the system should do")
  └── BDD Requirements (concrete — "Given X, When Y, Then Z")
       └── Plans / Implementation (how it gets built)
```

The matrix is authored first. Requirements fill it in. Gaps are immediately visible.

## Two Layers

Each layer forms its own matrix with a natural column axis:

### Invariant Layer

Rules that must always hold, mapped against domain entities.

- **Rows**: Domain invariants (e.g., "Cascade deletes to children", "Title is required", "Must belong to at least one codebase")
- **Columns**: Domain entities (e.g., Chunk, Plan, Requirement, Feature, Connection, Tag)
- **Cell**: Whether this invariant applies to this entity

### Contract Layer

Capabilities the system offers, mapped against actors/contexts.

- **Rows**: Behavioral contracts (e.g., "Search by tag", "Bulk delete", "Export to Gherkin")
- **Columns**: Actors/contexts (e.g., User, AI Agent, System/Cron, API Consumer)
- **Cell**: Whether this capability is available to this actor

## Cell Semantics

Cells are status flags, not content. The detail lives in linked requirements.

| State           | Meaning                                                 | Color  |
| --------------- | ------------------------------------------------------- | ------ |
| **Specified**   | Cell has linked requirements, none failing              | Green  |
| **Unspecified** | Cell exists but has no linked requirements (a gap)      | Yellow |
| **Violated**    | Cell has at least one failing requirement               | Red    |
| **N/A**         | No cell at this intersection (intentionally irrelevant) | Gray   |

The distinction between "unspecified" (yellow) and "N/A" (gray) is critical: an unspecified cell is a declared gap — "this should be
specified but isn't yet." A missing cell means "this intersection doesn't matter."

Status is computed at query time from linked requirement statuses, never stored.

## Data Model

### `behavior_matrix`

| Column        | Type                    | Notes                     |
| ------------- | ----------------------- | ------------------------- |
| `id`          | text PK                 | nanoid                    |
| `name`        | text, required          | e.g., "Domain Invariants" |
| `layer`       | text, required          | `invariant` \| `contract` |
| `description` | text, nullable          |                           |
| `codebaseId`  | FK → codebase, nullable | optional scoping          |
| `userId`      | FK → user, required     | owner                     |
| `createdAt`   | timestamp               |                           |
| `updatedAt`   | timestamp               |                           |

### `behavior_dimension`

Column in the matrix (entity or actor).

| Column      | Type                 | Notes                     |
| ----------- | -------------------- | ------------------------- |
| `id`        | text PK              | nanoid                    |
| `matrixId`  | FK → behavior_matrix | cascade delete            |
| `name`      | text, required       | e.g., "Chunk", "AI Agent" |
| `order`     | integer              | display order             |
| `createdAt` | timestamp            |                           |

Unique constraint: `(matrixId, name)`.

### `behavior_rule`

Row in the matrix (invariant or contract).

| Column        | Type                 | Notes                               |
| ------------- | -------------------- | ----------------------------------- |
| `id`          | text PK              | nanoid                              |
| `matrixId`    | FK → behavior_matrix | cascade delete                      |
| `title`       | text, required       | e.g., "Cascade deletes to children" |
| `description` | text, nullable       | brief elaboration                   |
| `category`    | text, nullable       | grouping within the matrix          |
| `order`       | integer              | display order                       |
| `createdAt`   | timestamp            |                                     |
| `updatedAt`   | timestamp            |                                     |

### `behavior_cell`

Explicit intersection of a rule and a dimension.

| Column        | Type                    | Notes          |
| ------------- | ----------------------- | -------------- |
| `id`          | text PK                 | nanoid         |
| `ruleId`      | FK → behavior_rule      | cascade delete |
| `dimensionId` | FK → behavior_dimension | cascade delete |
| `createdAt`   | timestamp               |                |

Unique constraint: `(ruleId, dimensionId)`.

### `behavior_cell_requirement`

Links cells to BDD requirements.

| Column          | Type                      | Notes          |
| --------------- | ------------------------- | -------------- |
| `cellId`        | FK → behavior_cell        | cascade delete |
| `requirementId` | FK → requirement          | cascade delete |
| PK              | `(cellId, requirementId)` |                |

## API Endpoints

### Matrix CRUD

| Method   | Path                | Notes                                |
| -------- | ------------------- | ------------------------------------ |
| `GET`    | `/api/matrices`     | List (filter: `codebaseId`, `layer`) |
| `POST`   | `/api/matrices`     | Create                               |
| `GET`    | `/api/matrices/:id` | Detail                               |
| `PATCH`  | `/api/matrices/:id` | Update name/description              |
| `DELETE` | `/api/matrices/:id` | Cascade all children                 |

### Dimensions

| Method   | Path                                   | Notes          |
| -------- | -------------------------------------- | -------------- |
| `POST`   | `/api/matrices/:id/dimensions`         | Add column     |
| `PATCH`  | `/api/matrices/:id/dimensions/:dimId`  | Rename         |
| `DELETE` | `/api/matrices/:id/dimensions/:dimId`  | Cascades cells |
| `POST`   | `/api/matrices/:id/dimensions/reorder` | Reorder        |

### Rules

| Method   | Path                              | Notes          |
| -------- | --------------------------------- | -------------- |
| `POST`   | `/api/matrices/:id/rules`         | Add row        |
| `PATCH`  | `/api/matrices/:id/rules/:ruleId` | Update         |
| `DELETE` | `/api/matrices/:id/rules/:ruleId` | Cascades cells |
| `POST`   | `/api/matrices/:id/rules/reorder` | Reorder        |

### Cells

| Method   | Path                                                  | Notes                                                                                                                                                                                 |
| -------- | ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PUT`    | `/api/matrices/:id/cells`                             | Upsert/delete cell (body: `ruleId`, `dimensionId`). Creates if absent, deletes if present AND has no linked requirements. Returns 409 if cell has linked requirements — unlink first. |
| `POST`   | `/api/matrices/:id/cells/:cellId/requirements`        | Link requirement                                                                                                                                                                      |
| `DELETE` | `/api/matrices/:id/cells/:cellId/requirements/:reqId` | Unlink                                                                                                                                                                                |

### Matrix View

| Method | Path                     | Notes                            |
| ------ | ------------------------ | -------------------------------- |
| `GET`  | `/api/matrices/:id/view` | Full grid with computed statuses |

Response shape:

```json
{
    "matrix": { "id": "...", "name": "Domain Invariants", "layer": "invariant" },
    "dimensions": [{ "id": "...", "name": "Chunk", "order": 0 }],
    "rules": [{ "id": "...", "title": "Cascade deletes", "category": "Lifecycle", "order": 0 }],
    "cells": {
        "rule-1:dim-1": { "id": "...", "status": "specified", "requirementCount": 2 },
        "rule-1:dim-2": null,
        "rule-2:dim-1": { "id": "...", "status": "unspecified", "requirementCount": 0 }
    },
    "summary": { "specified": 12, "unspecified": 5, "violated": 1, "total": 18 }
}
```

## UI

### Pages

**`/matrices`** — list page

- Cards per matrix: name, layer badge, coverage bar (stacked green/yellow/red), codebase scope
- Create button → form (name, layer, optional codebase)

**`/matrices/:id`** — matrix view (core experience)

- Grid layout: dimensions as column headers, rules as row headers, cells at intersections
- Cell colors: green/yellow/red/gray per status
- Click a cell → slide-over panel: linked requirements with status badges, "Link Requirement" button
- Click an empty (gray) intersection → creates a cell (turns yellow — declares a gap)
- Right-click a cell → remove cell (turns gray — marks as irrelevant)
- Inline row/column addition: "add rule" input at bottom, "add dimension" input on right
- Category grouping: collapsible section headers within the grid for rules
- Sticky header with coverage summary: "14/20 specified, 4 unspecified, 2 violated"

### Integration with Existing Pages

- `/requirements/:id` detail: section listing matrix cells this requirement satisfies
- `/coverage` page: link to matrix views
- Nav sidebar: "Matrices" link between Requirements and Coverage

### Interaction Flow

```
1. Author defines matrix (name + layer)
2. Adds dimensions (entities or actors)
3. Adds rules (invariants or contracts)
4. Clicks intersections to create cells ("this matters")
5. Links requirements to cells
6. Matrix view shows gaps (yellow) and violations (red)
```

## MCP Tools

| Tool                    | Purpose                                      |
| ----------------------- | -------------------------------------------- |
| `list_matrices`         | List available matrices (filter by codebase) |
| `get_matrix_view`       | Full grid with computed statuses             |
| `create_matrix`         | Create a new matrix                          |
| `add_dimension`         | Add a column                                 |
| `add_rule`              | Add a row                                    |
| `toggle_cell`           | Mark an intersection as relevant             |
| `link_cell_requirement` | Connect a requirement to a cell              |
| `get_matrix_gaps`       | Return only unspecified/violated cells       |

## CLI Commands

```
fubbik matrix list
fubbik matrix create <name> --layer invariant|contract [--codebase <name>]
fubbik matrix show <id>                              # ASCII grid with colored status
fubbik matrix add-dimension <id> <name>
fubbik matrix add-rule <id> <title> [--category <cat>]
fubbik matrix cell <id> <ruleId> <dimId>             # toggle cell
fubbik matrix gaps <id>                              # list unspecified/violated cells
fubbik matrix link <cellId> <requirementId>
```

## CLAUDE.md Integration

The `sync-claude-md` command includes a matrix summary section:

```markdown
## Behavioral Coverage

### Domain Invariants — 78% specified (14/18), 1 violated

| ⚠ VIOLATED | "Cascade deletes" × "Feature" — requirement R-42 failing

### API Contracts — 60% specified (12/20), 0 violated

| GAP | "Bulk delete" × "AI Agent" — unspecified | GAP | "Export to Gherkin" × "System" — unspecified
```

## Architecture

Follows existing patterns:

- **Schema**: `packages/db/src/schema/behavior-matrix.ts` — all five tables
- **Repository**: `packages/db/src/repository/behavior-matrix.ts` — returns `Effect<T, DatabaseError>`
- **Service**: `packages/api/src/matrices/service.ts` — business logic, status computation
- **Routes**: `packages/api/src/matrices/routes.ts` — Elysia routes with auth
- **Frontend**: `apps/web/src/features/matrices/` — list page, matrix grid view, cell panel
- **MCP**: `packages/mcp/src/matrix-tools.ts` — `registerMatrixTools(server)`
- **CLI**: `apps/cli/src/commands/matrix.ts` — Commander.js subcommands

## Non-Goals (v1)

- Matrix versioning/history (rules and dimensions change rarely enough to not need it)
- Auto-generating matrices from existing requirements (could be a v2 AI feature)
- Cross-matrix dependencies (each matrix is self-contained)
- Permissions beyond user ownership (follows existing auth model)
