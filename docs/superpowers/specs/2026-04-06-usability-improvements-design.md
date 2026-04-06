# Usability Improvements: Staleness, Relationships, Quick Capture

## Problem

Three core usability pain points prevent Fubbik from being a living knowledge system:

1. **Chunks go stale silently** — code changes make chunks outdated but nothing alerts the user. Chunks sit forgotten for months. Low health scores don't trigger action. Duplicates diverge unnoticed.
2. **Relationships are hard to see** — the graph is noisy with many nodes, dependency direction isn't clear from detail pages, and cross-codebase connections are invisible without deliberate searching.
3. **Capturing knowledge requires context switching** — knowledge discovered in editors, terminals, or chat has to be manually re-entered into Fubbik's web UI.

## Design

### 1. Staleness — "Never Let Chunks Rot Silently"

#### 1a. Git-aware staleness detection

A background process compares recent git diffs against chunk `appliesTo` globs and `fileReferences`. When linked files change, the chunk is flagged as "possibly stale."

**Data model changes:**
- New `chunk_staleness` table:
  - `chunkId` (FK to chunk)
  - `reason` enum: `file_changed`, `age`, `diverged_duplicate`
  - `detail` (text — e.g., which files changed, or which chunk is the duplicate)
  - `detectedAt` (timestamp)
  - `dismissedAt` (nullable timestamp — user reviewed and dismissed)

**Detection triggers:**
- **File changes**: CLI hook (`fubbik hooks`) or periodic scan. Runs `git diff --name-only` against last known scan commit, matches changed paths against chunk `appliesTo` globs and `fileReferences`. Flags matching chunks with `file_changed`. Last scanned commit SHA stored in a `staleness_scan` table (per codebase) to make scans incremental and idempotent.
- **Age**: Chunks not updated in 90+ days flagged with `age`. Configurable threshold.
- **Duplicate divergence**: Periodic embedding similarity scan. Chunk pairs with cosine similarity > 0.90 flagged with `diverged_duplicate`. Runs during enrichment or as a scheduled background task.

**API:**
- `GET /api/chunks/stale` — list stale chunks with reasons, supports filtering by reason
- `POST /api/chunks/:id/dismiss-staleness` — dismiss a staleness flag
- `GET /api/chunks/stale/count` — count for nav badge

#### 1b. Staleness surfacing across the UI

**Chunk list page:**
- Amber dot indicator next to stale chunk titles
- New filter option: "Stale" (shows only flagged chunks)
- New sort option: "Staleness" (most stale first)

**Dashboard "Attention Needed" widget:**
- New section above or alongside existing dashboard content
- Shows top 10 stale chunks grouped by reason:
  - "Files changed" — lists chunk title + which files changed + days since change
  - "Getting old" — chunks not updated in 90+ days
  - "Possible duplicates" — pairs of similar chunks
- Each item has:
  - Click to navigate to chunk detail
  - "Mark reviewed" button (dismisses staleness flag)
  - "Edit" quick link

**Nav badge:**
- Small amber count badge on a nav item (e.g., on "Dashboard" or a dedicated "Attention" link)
- Shows total undismissed stale chunk count
- Disappears when count is 0

**Chunk detail page:**
- Warning banner at top when chunk is stale:
  - `file_changed`: "Files linked to this chunk changed 3 days ago: `src/auth/middleware.ts`, `src/auth/session.ts`" with link to git diff
  - `age`: "This chunk hasn't been updated in 4 months"
  - `diverged_duplicate`: "This chunk is very similar to [Other Chunk Title]" with link
- "Dismiss" button on the banner to mark as reviewed

#### 1c. Duplicate/divergence detection

- Runs as part of the existing embedding pipeline (when embeddings are generated/refreshed)
- Compares each chunk's embedding against all others in the same codebase
- Pairs above 0.90 cosine similarity are flagged
- Shown on:
  - Dashboard "Attention Needed" widget
  - Knowledge health page (existing `/knowledge-health`)
  - Chunk detail page banner
- Actions: "Merge" (opens comparison view at `/compare`), "Dismiss" (marks as reviewed), "They're different" (permanently suppresses this pair)

### 2. Relationships — "See Connections Without Hunting"

#### 2a. Chunk detail: inline relationship view

**Dependency tree (replaces flat connection list):**
- Two-column layout or grouped sections: "Depends on" (outgoing) and "Used by" (incoming)
- Each connection shows: chunk title, relation type badge, chunk type icon
- Grouped by relation type (part_of, depends_on, extends, etc.)
- Click to navigate, hover for content preview (reuse existing tooltip)

**"Same files" section:**
- Automatically computed — query chunks sharing overlapping `fileReferences` or `appliesTo` glob matches
- Shown as a compact list: "These chunks also reference `src/auth/*`:"
- No manual connection needed — purely computed
- Collapsed by default if more than 5 items

**"Related chunks" suggestions:**
- Powered by embedding similarity (top 3-5, excluding already-connected chunks)
- Shown as a dismissable card: "You might want to connect these"
- Each suggestion has: title, similarity score (as a subtle percentage), "Link" button (opens relation picker), "Dismiss" button
- Dismissed suggestions stored in localStorage to avoid re-showing

#### 2b. Graph improvements

**Codebase clustering:**
- In workspace view, nodes are grouped into labeled bounding regions per codebase
- Edges crossing codebase boundaries are styled differently (dashed, distinct color)
- Each cluster has a subtle background color and label

**Focus mode:**
- Click a node to enter focus mode: everything beyond 2 hops is dimmed (reduced opacity)
- Focused subgraph is highlighted with full opacity
- Click background or press Escape to exit focus mode
- Works with existing layout algorithms

**Filter presets:**
- "Save current filters" button in the graph toolbar
- Presets stored in localStorage with user-defined names
- Dropdown to select/apply/delete presets
- Default presets: "All", "Architecture only", "Current codebase"

#### 2c. Cross-codebase visibility

**Chunk detail page:**
- If the chunk's `fileReferences` or content overlaps with chunks in other codebases, show a "Cross-codebase" section
- Lists: codebase name + chunk titles from other codebases that reference similar files
- Leverages the existing federated search infrastructure

**Search results:**
- Codebase name badge already exists in federated search — make it more prominent with color-coded badges matching graph cluster colors

### 3. Quick Capture — "Jot It Down Without Leaving Your Flow"

#### 3a. Command palette quick-add

**Extend existing Cmd+K palette:**
- New action at top of results: "Quick note: {typed text}" (appears when input doesn't match a page/command)
- Selecting it creates a `note` chunk with:
  - Title: the typed text
  - Type: `note`
  - Codebase: current active codebase from nav context
  - All other fields empty
- Shows success toast: "Created '{title}'" with "Edit" link to `/chunks/:id/edit`
- No form, no intermediate step — immediate creation

#### 3b. Clipboard-to-chunk

**New keyboard shortcut: `Shift+N`**
- Reads clipboard content
- Opens `/chunks/new` with content pre-populated from clipboard
- If clipboard content is markdown, it's placed in the content field as-is
- If clipboard starts with a `# Heading`, that heading is used as the title
- User adds title/tags/type before saving — this is not instant creation, just pre-population

#### 3c. CLI one-liner

**New `fubbik quick` command:**

```
fubbik quick "Convention: always use Effect for errors"
```

- Creates a `note` chunk with the argument as the title
- Auto-detects codebase from git remote in current directory
- Prints chunk ID and web URL on success

**Pipe support:**

```
cat DECISIONS.md | fubbik quick --title "Architecture Decisions"
```

- Reads stdin as content
- `--title` flag required when piping
- `--type` flag optional (defaults to `note`)
- `--tags` flag optional (comma-separated)

## Out of Scope

- Email/Slack notifications for staleness (keep it in-app for now)
- Automated chunk updates based on git diffs (flag only, human reviews)
- Real-time collaborative editing
- Mobile-native app

## Implementation Notes

- Staleness detection should be idempotent — running it twice with the same git state produces no new flags
- The `chunk_staleness` table uses soft-dismiss (timestamp) rather than deletion so we can track patterns
- Embedding similarity for suggestions reuses the existing `pgvector` cosine distance operator
- Graph focus mode can be implemented with React Flow's node/edge style overrides (opacity changes)
- Filter presets are client-side only (localStorage) — no backend needed
- `fubbik quick` reuses the existing `POST /api/chunks` endpoint with minimal fields
