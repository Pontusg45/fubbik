# UX & AI Improvements Roadmap

## Overview

A phased plan covering human UX, AI agent integration, and shared improvements across fubbik. Organized into 6 phases by dependency and impact.

---

## Phase 1: Navigation & Command Palette

**Priority:** High — improves daily usability immediately.

### Command Palette (`Cmd+K`)
- Global search across chunks, requirements, vocabulary, templates
- Action shortcuts: create chunk, navigate to page, switch codebase
- Results grouped by type with keyboard navigation
- Implementation: React component in root layout, listens for `Cmd+K`/`Ctrl+K`
- Search hits the existing `GET /api/chunks?search=` endpoint + client-side page matching

### Breadcrumbs
- Show navigation path: `Fubbik > codebase > section > item`
- Auto-generated from TanStack Router route hierarchy
- Clickable segments to navigate back up

### Keyboard Shortcuts
- `/` — focus search
- `n` — create new (context-aware: chunk on chunks page, requirement on requirements page)
- `e` — edit current item
- `Esc` — go back / close dialogs
- `j`/`k` — navigate lists (already exists on chunks page, extend everywhere)
- Help overlay (`?`) showing all shortcuts

### Favorites/Pinning (Server-side)
- Move from localStorage to a `user_favorite` table: `userId`, `chunkId`, `order`
- Show pinned chunks in sidebar or dashboard
- `Cmd+D` to toggle favorite on chunk detail page

---

## Phase 2: Content Quality & Editing

**Priority:** High — makes writing and reading chunks much better.

### Markdown Preview
- Side-by-side or toggle preview on chunk create/edit pages
- Use `react-markdown` or `@uiw/react-md-editor` (check which is lighter)
- Preview renders in real-time as user types

### Chunk Size Indicator
- Visual progress bar on create/edit showing content length
- Zones: thin (<100 chars, red), short (100-500, yellow), good (500-2000, green), verbose (>2000, blue)
- Ties into the knowledge health "thin chunks" concept

### Related Chunks Sidebar
- On chunk detail page, show "Related" panel
- Source: direct connections + chunks sharing tags + chunks in same codebase with similar titles
- Collapsed by default, expandable

### Link Preview on Hover
- When hovering a chunk link anywhere in the app, show a preview card
- Card shows: title, type badge, first 2 lines of content, tags
- Delay: 300ms hover before showing
- Implementation: shared `ChunkPreviewCard` component + Popover

### Diff Viewer
- On chunk history page, select two versions and see side-by-side diff
- Use a lightweight diff library (`diff` npm package)
- Highlight additions (green) and deletions (red)

---

## Phase 3: Organization & Bulk Operations

**Priority:** Medium — power user features for managing larger knowledge bases.

### Bulk Operations
- Multi-select mode on chunks page (checkbox per row, "Select all")
- Bulk actions: add tags, move to codebase, change type, delete, change review status
- Confirmation dialog showing what will change
- API: `POST /api/chunks/bulk-update` with `{ ids[], action, value }`

### Smart Collections
- Saved filters that auto-update
- New `collection` table: `id`, `name`, `filter` (JSONB storing query params), `userId`, `codebaseId`
- UI: "Save current filters as collection" button on chunks page
- Collections appear in sidebar or as tabs

### Chunk Archives (Soft Delete)
- Add `archivedAt` timestamp (nullable) to chunk table
- Archived chunks are excluded from normal queries but still exist
- `/chunks/archived` page to view and restore
- "Archive" replaces "Delete" as default action, "Delete permanently" is secondary

### Drag-and-Drop Tag Assignment
- Kanban view on chunks page (already has view toggle)
- Drag chunks between tag columns to reassign
- Uses React DnD or similar

---

## Phase 4: AI Agent Integration

**Priority:** High — makes fubbik useful for AI workflows.

### MCP Server
- New package: `packages/mcp/` exposing fubbik as an MCP tool server
- Tools: `search_chunks`, `get_chunk`, `create_chunk`, `get_conventions`, `get_requirements`, `check_vocabulary`
- Connects to fubbik API via HTTP (same as VS Code extension pattern)
- Configuration: server URL in MCP config
- Supports Claude Code, Cursor, and any MCP-compatible client

### Token-Aware Context Export
- `GET /api/chunks/export/context?codebaseId=&maxTokens=4000&format=markdown|json`
- Ranks chunks by priority score: `connectionCount * 2 + (isConvention ? 5 : 0) + recencyScore`
- Greedily fills token budget with highest-priority chunks
- Token estimation: ~4 chars per token (rough but fast)
- CLI: `fubbik context --max-tokens 4000 --codebase myproject`

### `.fubbik/context.md` Auto-Generation
- CLI command: `fubbik generate context` — writes `.fubbik/context.md` in the repo
- Optional git hook: `fubbik hook install` adds a pre-commit hook that regenerates if chunks changed
- Content: top-priority chunks formatted as a system prompt
- AI tools that read project files (Cursor, Copilot) get fubbik knowledge automatically

### CLAUDE.md / AGENTS.md Generator
- `fubbik generate claude.md --codebase myproject > CLAUDE.md`
- Structured output: Project Overview, Tech Stack, Architecture, Conventions, Commands
- Sources from chunk types: architecture chunks → Architecture section, convention chunks → Conventions, etc.
- Uses `appliesTo` and `fileReferences` to organize by relevance
- API: `GET /api/codebases/:id/generate-instructions?format=claude|agents|cursor`

### Machine-Readable Chunk Format
- `GET /api/chunks/export/structured?codebaseId=` returns JSON with full metadata:
  ```json
  {
    "chunks": [{
      "title": "...",
      "content": "...",
      "type": "convention",
      "appliesTo": [{ "pattern": "src/auth/**" }],
      "fileReferences": [{ "path": "src/auth/session.ts", "anchor": "SessionManager" }],
      "connections": [{ "target": "...", "relation": "depends_on" }],
      "tags": ["backend", "auth"],
      "origin": "human",
      "reviewStatus": "approved"
    }]
  }
  ```
- AI agents parse this to understand exactly what conventions apply to which files

---

## Phase 5: AI as Contributor

**Priority:** Medium — AI actively helps maintain the knowledge base.

### AI Chunk Suggestions from Code Changes
- `POST /api/ai/suggest-from-diff` with a git diff or PR description
- AI analyzes what changed, suggests new chunks or updates to existing ones
- Returns: `{ newChunks: [...], updatedChunks: [{ id, suggestedChanges }] }`
- Web UI: review page showing suggestions with accept/reject
- CLI: `fubbik ai suggest --from-diff HEAD~1`

### Auto-Connection Suggestions
- After creating/editing a chunk, `POST /api/ai/suggest-connections?chunkId=`
- Uses embedding similarity to find top-5 most related chunks
- Returns suggestions with similarity scores
- UI: "Suggested connections" section on chunk detail, one-click to link

### Semantic Stale Detection
- Beyond date-based: compare chunk content against current code via embeddings
- `POST /api/ai/check-staleness?chunkId=` embeds the chunk + its referenced files, measures drift
- If embedding distance is high, chunk content may no longer match the code
- Integrated into knowledge health page as an additional section

### AI-Assisted Requirement Writing
- On requirement create page: "Describe what you want" text box
- Sends to AI, returns structured Given/When/Then steps using the codebase vocabulary
- User reviews and edits before saving
- `POST /api/ai/structure-requirement` with `{ description, codebaseId }`

### Convention Enforcement API
- `POST /api/check` with `{ code, filePath, codebaseId }`
- Finds applicable conventions (via `appliesTo` patterns matching filePath)
- AI checks if the code follows the conventions
- Returns: `{ applicable: [...], violations: [...], suggestions: [...] }`
- Useful for CI integration or editor plugins

---

## Phase 6: Shared UX Polish

**Priority:** Medium — polish that benefits everyone.

### Notification System
- `notification` table: `id`, `userId`, `type`, `title`, `message`, `read`, `createdAt`
- Types: stale_chunks, review_needed, ai_suggestion, chunk_updated
- Bell icon in nav header with unread count
- Notifications page showing all notifications
- Generated by: health checks (cron), AI suggestions, review status changes

### Dashboard Widgets
- Configurable dashboard replacing the current fixed layout
- Widget types: recent activity, health summary, review queue, codebase stats, favorites
- `dashboard_widget` table: `userId`, `widgetType`, `order`, `config` (JSONB)
- Drag-and-drop reorder

### Unified Search
- `/search?q=` page showing results across all entity types
- Results grouped: Chunks, Requirements, Templates, Vocabulary entries
- Each result shows: title, type badge, relevance score, excerpt
- Powers the command palette search

### Activity Feed
- `/activity` page showing chronological changes
- Sources: chunk create/update/delete, requirement changes, connection changes
- `activity_log` table or derived from `chunk_version` + other entity timestamps
- Filterable by codebase, entity type, user

### Mobile Responsive
- Responsive breakpoints for all pages
- List views collapse to single-column cards
- Graph page: simplified touch-friendly controls
- Read-only focus on mobile (create/edit works but isn't optimized)

### Dark Mode Graph Improvements
- Better edge contrast on dark backgrounds
- Node text readability improvements
- Group node backgrounds tuned for dark theme
- Legend panel with dark-mode colors

---

## Implementation Order

```
Phase 1: Navigation & Command Palette     ← start here (highest daily impact)
Phase 4: AI Agent Integration             ← parallel (independent, high value for AI)
Phase 2: Content Quality & Editing        ← after Phase 1
Phase 5: AI as Contributor                ← after Phase 4
Phase 3: Organization & Bulk Operations   ← after Phase 2
Phase 6: Shared UX Polish                 ← ongoing alongside others
```

Each feature within a phase gets its own spec → plan → implementation cycle. Features within a phase can be parallelized if they touch different files.

## Estimated Scope

| Phase | Features | Rough Size |
|-------|----------|-----------|
| 1 | Command palette, breadcrumbs, shortcuts, favorites | Medium |
| 2 | Markdown preview, size indicator, related sidebar, link preview, diff viewer | Medium |
| 3 | Bulk ops, smart collections, archives, drag-drop tags | Large |
| 4 | MCP server, token-aware export, context.md gen, CLAUDE.md gen, structured export | Large |
| 5 | AI suggestions, auto-connections, semantic staleness, AI requirements, convention enforcement | Large |
| 6 | Notifications, dashboard widgets, unified search, activity feed, mobile, dark mode | Large |
