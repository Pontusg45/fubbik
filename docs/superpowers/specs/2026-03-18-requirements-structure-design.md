# Requirements Structure & Organization

## Overview

Add sub use cases (single-level nesting), requirement dependencies with a dependency graph, and drag-to-reorder requirements within use cases.

## 1. Sub Use Cases (Single-Level Nesting)

### Schema

Add `parentId` column to `use_case` table:
- `parent_id TEXT REFERENCES use_case(id) ON DELETE CASCADE` (nullable)
- Index: `index("use_case_parentId_idx").on(table.parentId)` in the table builder array

Parents cannot have parents — enforced in the service layer (reject if the target parent already has a `parentId`).

Update `useCaseRelations` to add self-referential relations:
- `parent: one(useCase, { fields: [useCase.parentId], references: [useCase.id], relationName: "parent" })`
- `children: many(useCase, { relationName: "parent" })`

### API Changes

- `POST /api/use-cases` — add optional `parentId` field. Validates parent exists and has no parent of its own.
- `GET /api/use-cases` — returns flat list with `parentId` field. Include `children` count for parent use cases.
- `PATCH /api/use-cases/:id` — allow changing `parentId` (move to different parent or promote to top-level by setting null). Validate no circular reference and target is not itself a child.
- `DELETE /api/use-cases/:id` — cascade deletes children (DB handles via ON DELETE CASCADE). Requirements in deleted children become ungrouped (handled by the requirement table's `ON DELETE SET NULL` FK on `use_case_id`).

### Repository

- `listUseCases` — add `parentId` to select, add `childCount` via left join subquery.
- `createUseCase` / `updateUseCase` — accept and persist `parentId`.

### Frontend (Sidebar)

The sidebar's use case list becomes a tree:
- Top-level use cases shown normally
- Clicking a parent expands/collapses to show children indented underneath
- Children show with indentation (left padding) and slightly smaller text
- Clicking a child use case filters to just that child's requirements (single `useCaseId`)
- Clicking a parent use case filters to all requirements in that parent AND its children — frontend fetches the parent's child IDs from the use cases list and sends multiple requests or filters client-side
- Requirement counts include children's requirements for parent use cases (computed client-side from the flat list)

### Constraints

- Unique constraint `(userId, name)` remains — names unique across all use cases regardless of nesting
- Maximum nesting depth: 1 (parent → child, no deeper)

## 2. Requirement Dependencies

### Schema

New `requirement_dependency` table:
- `requirement_id TEXT NOT NULL REFERENCES requirement(id) ON DELETE CASCADE`
- `depends_on_id TEXT NOT NULL REFERENCES requirement(id) ON DELETE CASCADE`
- Primary key: `(requirement_id, depends_on_id)`
- Check constraint: `requirement_id != depends_on_id` (no self-dependencies)

### Repository

- `addDependency(requirementId, dependsOnId)` → `Effect<void, DatabaseError>`
- `removeDependency(requirementId, dependsOnId)` → `Effect<void, DatabaseError>`
- `getDependencies(requirementId)` → `Effect<{ dependsOn: Requirement[], dependedOnBy: Requirement[] }, DatabaseError>`
- `getTransitiveDependencies(requirementId)` → `Effect<{ ancestors: Requirement[], descendants: Requirement[] }, DatabaseError>` — recursive CTE query for the graph

### Service

- `addDependency(requirementId, dependsOnId, userId)` — validates both requirements exist and belong to user. Checks for circular dependencies by walking the transitive chain of `dependsOnId` — if `requirementId` appears anywhere in that chain, reject with a ValidationError.
- `removeDependency(requirementId, dependsOnId, userId)` — validates ownership.
- `getDependencies(requirementId, userId)` — returns direct dependencies in both directions.
- `getDependencyGraph(requirementId, userId)` — returns the full transitive chain (ancestors + descendants) with edges for the React Flow graph.

### API

- `POST /api/requirements/:id/dependencies` — body: `{ dependsOnId: string }`. Returns 201.
- `DELETE /api/requirements/:id/dependencies/:dependsOnId` — returns 200.
- `GET /api/requirements/:id/dependencies` — returns `{ dependsOn: [...], dependedOnBy: [...] }` (direct only).
- `GET /api/requirements/:id/dependencies/graph` — returns `{ nodes: [...], edges: [...] }` for React Flow. Nodes include requirement id, title, status. Edges include source/target.

### Frontend (Detail Page)

The detail page fetches dependency data via a separate `GET /api/requirements/:id/dependencies` call (not inlined in the main requirement response). This keeps the main query fast and the dependency section independent.

**Dependencies section** (below steps, above warnings):
- Two sub-sections: "Depends on" and "Depended on by"
- Each shows a list of linked requirements as clickable cards with title and status badge
- "Add dependency" button opens a search-and-select UI (same pattern as ChunkLinker) to pick requirements
- Remove button (x) on each dependency link

**Dependency graph** (collapsible section):
- "View dependency graph" button that expands a React Flow graph panel
- Nodes are requirements colored by status (green=passing, red=failing, gray=untested)
- The current requirement node is highlighted (thicker border or different shape)
- Edges are arrows pointing from dependent → prerequisite (direction = "depends on")
- Layout: dagre top-to-bottom (directional DAG)
- Clicking a node navigates to that requirement's detail page
- Graph fetches from `GET /api/requirements/:id/dependencies/graph`

### Circular Dependency Prevention

When adding a dependency A → B ("A depends on B"):
1. Fetch all transitive dependencies of B (everything B depends on, recursively)
2. If A appears in that set, reject — adding this would create a cycle
3. Use a recursive CTE in PostgreSQL for efficiency

## 3. Ordering Within Use Cases

### Schema

Add `order` column to `requirement` table:
- `order INTEGER NOT NULL DEFAULT 0`

Requirements are sorted by `order ASC, created_at ASC` (order first, creation date as tiebreaker).

### Repository

- `reorderRequirements(requirementIds: string[], userId: string)` → `Effect<void, DatabaseError>` — sets `order = index` for each requirement in the array. Uses a single transaction.
- `listRequirements` — update to sort by `order ASC, created_at ASC` (add `.orderBy(asc(requirement.order), asc(requirement.createdAt))`).

### Service

- `reorderRequirements(requirementIds, userId)` — fetches all requirements by ID in a single query (`WHERE id IN (...)`), validates they all exist, belong to user, and share the same `useCaseId` (or are all null/ungrouped). Calls the repository function.

### API

- `PATCH /api/requirements/reorder` — body: `{ requirementIds: string[] }`. Returns 200 with `{ updated: number }`.

Must be registered before `/:id` routes in Elysia to avoid parameter collision (same pattern as the bulk route).

### Frontend (List Page)

- Add `@dnd-kit/core` (v6.x stable) and `@dnd-kit/sortable` (v8.x stable) as dependencies to the web app
- Each requirement card gets a drag handle (grip/dots icon) on the left side
- Drag-and-drop reordering within a use case group (or within ungrouped)
- On drop, calls `PATCH /api/requirements/reorder` with the new order
- Optimistic update: reorder in the UI immediately, revert on error
- The sidebar use case sections and ungrouped section each act as independent sortable containers

### Migration

Generate a Drizzle migration for:
- `use_case.parent_id` column
- `requirement_dependency` table
- `requirement.order` column

## Files to Create/Modify

### New files
- `packages/db/src/schema/requirement-dependency.ts` — dependency table schema (must be added to `packages/db/src/schema/index.ts` barrel export)
- `packages/db/src/repository/requirement-dependency.ts` — dependency CRUD + transitive queries
- `packages/api/src/requirements/dependency-routes.ts` — dependency API endpoints
- `packages/api/src/requirements/dependency-service.ts` — dependency business logic
- `apps/web/src/features/requirements/dependency-section.tsx` — detail page dependency UI
- `apps/web/src/features/requirements/dependency-graph.tsx` — React Flow dependency visualization
- `apps/web/src/features/requirements/sortable-requirement-list.tsx` — drag-and-drop wrapper

### Modified files
- `packages/db/src/schema/use-case.ts` — add `parentId` column
- `packages/db/src/repository/use-case.ts` — add parentId support, child counts
- `packages/api/src/use-cases/routes.ts` — add parentId to create/update schemas
- `packages/api/src/use-cases/service.ts` — add nesting validation (if exists)
- `packages/api/src/requirements/routes.ts` — add reorder endpoint
- `packages/api/src/requirements/service.ts` — add reorder logic
- `packages/db/src/repository/requirement.ts` — add order to queries, reorder function
- `apps/web/src/features/requirements/sidebar-filters.tsx` — tree view for use cases
- `apps/web/src/routes/requirements.tsx` — integrate drag-and-drop
- `apps/web/src/routes/requirements_.$requirementId.tsx` — add dependency section + graph

## Out of Scope

- Multi-level nesting (grandchildren)
- Automated dependency detection from step text
- Dependency impact analysis (what breaks if this requirement fails)
- Cross-codebase dependencies
