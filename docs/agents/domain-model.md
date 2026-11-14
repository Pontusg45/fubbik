# Domain model

Reference for CLAUDE.md. Read when touching chunks, spaces, connections, plans, requirements, matrices, features/overlays, staleness, or health scores (schema, tables, behavior).

## Core Concepts

### Chunks

The central entity — discrete units of knowledge (conventions, architecture decisions, runbooks, etc.). Each chunk has:

- `title`, `content`, `type` (note, document, reference, schema, checklist)
- `tags` (via normalized join table with tag types)
- `scope` (JSONB key-value metadata)
- `appliesTo` (glob patterns linking chunks to file areas, via `chunk_applies_to` table)
- `fileReferences` (explicit bidirectional links to files/symbols, via `chunk_file_ref` table)
- `rationale`, `alternatives`, `consequences` (optional "why" decision context fields)
- `embedding` (768-dim vector for semantic search)
- `embeddingUpdatedAt` (tracks when embedding was last refreshed)
- AI-generated: `summary`, `aliases`, `notAbout`
- Health score (computed on-demand): freshness + completeness + richness + connectivity (0-100)
- Version history (append-only `chunk_version` table)

### Spaces & Workspaces

Chunks can be organized per-space. A space is identified by a `kind` discriminator (`code | wiki | notes | research`) and — for `code`
kind spaces — a git remote URL (normalized) or local paths stored in the `space_code_metadata` side-table. Chunks can belong to multiple
spaces or none (global).

- `space` table with `remoteUrl` (deprecated, now in `space_code_metadata`), `localPaths`, `name`, `kind`
- `space_code_metadata` side-table holding `remoteUrl`, `localPaths`, `defaultBranch` for `kind = 'code'` spaces
- `chunk_space` join table (many-to-many; formerly `chunk_codebase`)
- `workspace` table groups related spaces (e.g., frontend + backend + infra)
- `workspace_space` join table (formerly `workspace_codebase`)
- `feature_space` join table (formerly `feature_codebase`)
- `fubbik space current` detects the space for the current directory via git remote or local path
- Web UI has a space/workspace switcher in the nav

### Connections

Directed edges between chunks: `sourceId → targetId` with a `relation` type (related_to, part_of, depends_on, extends, references, supports,
contradicts, alternative_to). Connections are global (not space-scoped), enabling cross-project knowledge linking.

### Templates

Built-in + user-created chunk templates (Decision Record, API Reference, Meeting Notes, Checklist, Schema). Built-in templates are seeded
by the `core` seed module (`crates/fubbik-db/src/seed/core-catalog.json`) and are read-only. Users can duplicate and customize.

### Plans

The central unit of work. Each plan holds a description, linked requirements, structured analyze fields, and enriched tasks.

- `plan` table: `title`, `description` (markdown), `status` (`draft | analyzing | ready | in_progress | completed | archived` — labels only,
  ungated), `userId`, `spaceId`, `completedAt`
- `plan_requirement` — many-to-many link to existing `requirement` entities at the plan level
- `plan_analyze_item` — discriminated table holding five kinds: `chunk`, `file`, `risk`, `assumption`, `question`, each with kind-specific
  metadata (severity for risks, verified flag for assumptions, answer for questions, line range for files)
- `plan_task` — enriched tasks with `title`, `description`, `acceptanceCriteria` (JSONB string array), `status`
- `plan_task_chunk` — many-to-many linking tasks to multiple chunks with a relation (`context | created | modified`)
- `plan_task_dependency` — task dependencies; marking a task `done` auto-unblocks dependents in `blocked` state
- Web UI: `/plans` list, `/plans/new` (simple form), `/plans/:id` (sticky header + four sections: description, requirements, analyze, tasks)
- CLI: `fubbik plan create/list/show/status/add-task/task-done/link-requirement`

### Agent Coordination Boards

Plans are also durable coordination boards for root agents and sub-agents:

- `agent_run` stores reconnectable Plan-scoped identities and parent/child relationships
- `plan_task_claim` stores exclusive expiring leases over existing `plan_task` rows
- `coordination_entry` is an append-only journal for notes, questions, progress, decisions, handoffs, and artifacts
- Rust module: `crates/fubbik-api/src/coordination/` with persistence in `crates/fubbik-db/src/repo/coordination.rs`
- MCP tools: `join_board`, `read_board`, `claim_task`, `update_board_task`, `write_board_entry`, `ack_board`
- The board persists communication but does not wake agent processes; agents poll or rely on their host for live delivery

### Requirements

BDD-style requirements with Given/When/Then steps.

- `requirement` table with `title`, `description`, `steps` (JSONB array of `{keyword, text}`), `status` (passing/failing/untested),
  `priority` (must/should/could/wont)
- `requirement_chunk` join table linking requirements to chunks
- Plans link to requirements through `plan_requirement` for traceability; requirement status is set explicitly (API, CLI `fubbik req
  status`, MCP `update_requirement_status`)

### Behavioral Matrices

Grid-based specs of expected behavior. A `behavior_matrix` (`layer = invariant | contract`) holds `behavior_dimension` columns and
`behavior_rule` rows; their intersection is a `behavior_cell`. Cells carry evidence and code links; cell status is computed on-the-fly.

- `behavior_rule` documents a behavior with `title`, `description`, `category`, plus decision-context fields `rationale`, `alternatives`,
  `consequences`, and an explicit `counterexample` (what violating it looks like).
- `behavior_rule_version` — append-only history; the service snapshots the pre-edit rule on every update (auditable evolution).
- `behavior_cell_requirement` — links a cell to BDD requirements (proof of behavior).
- `behavior_cell_code` — links a cell to concrete code (`kind = file | symbol | test`, `ref`). Powers reverse lookup
  (`GET /api/matrices/behaviors-for-file?path=`) and surfaces "Behaviors governing this file" in the file-context output.
- `behavior_test_result` — recorded `pass`/`fail` outcomes that drive verification status from real runs.
- Cell status (computed in the matrix view): `violated` (a failing requirement or failing test), `verified` (passing test evidence),
  `specified` (has requirements), `unspecified` (gap). The view also returns per-cell `codeCount`/`passingTestCount`/`failingTestCount`.
- Behavior rules are projected into the AGE graph as `behavior_rule` vertices with `governs` edges to `code_file`/`code_symbol`
  (`spawn_behavior_sync` in `crates/fubbik-api/src/graph/sync.rs`, started by `fubbik serve` when `matrices` is enabled); toggled via the
  "Behavior" button in the graph view.
- Surfaces: web `/matrices` + `/matrices/:id` (editable grid, cell panel, rule history dialog), CLI `fubbik matrix …`, and MCP matrix tools.

### Chunk Health Scores

Per-chunk health scores (0-100) computed on-demand from:

- **Freshness** (0-25): days since last update
- **Completeness** (0-25): has rationale/alternatives/consequences
- **Richness** (0-25): content length + AI enrichment
- **Connectivity** (0-25): number of connections Exposed via chunk detail API response and shown as a badge on the detail page.

### Features (Knowledge Overlays)

Named entities that track field-level modifications to chunks. Each feature stores deltas — sparse JSONB objects containing only the changed
fields. Features have a priority (higher wins on same-field conflicts) and can be toggled on/off globally via the nav switcher.

- `feature` table: `name`, `description`, `priority` (unique per user), `status` (inactive/active/merged/archived), `color`, `userId`
- `feature_space` join table (optional space association; formerly `feature_codebase`)
- `chunk_feature_delta` table: one delta per chunk per feature, contains JSONB with changed content fields only (title, content, type,
  rationale, alternatives, consequences, summary)
- `user_active_feature` table: persists which features are active per user
- Resolution: active deltas are folded field-by-field in ascending priority, so the highest priority wins — applied at the service layer
  (e.g. `resolve_feature_overlays` in `crates/fubbik-api/src/context/service.rs`)
- Merge: permanently applies all deltas to base chunks (with version snapshots), sets feature status to `merged`
- Feature switcher in nav bar alongside space switcher
- `/features` management page with create, merge, archive, delete actions

### Staleness Detection

Proactive detection of chunks that may need attention:

- `chunk_staleness` table tracks flags with `reason` (file_changed, age, diverged_duplicate), `detail`, and dismiss/suppress state
- `staleness_scan` table tracks per-space last scanned git commit SHA for incremental scans
- Age-based detection: flags chunks not updated in 90+ days (configurable threshold)
- Flags are dismissable (soft-delete with timestamp) or permanently suppressible (for duplicate pairs)
- Surfaced across the UI: dashboard "Attention Needed" widget, nav badge on Dashboard link, amber banners on chunk detail pages

### Scope Schema Registry

Optional registry for chunk scope keys. The `scope_key` table defines expected keys with types (string, number, boolean, enum) and allowed
values. Used for UI autocomplete and optional validation. Scope remains free-form JSONB — the registry is opt-in.

