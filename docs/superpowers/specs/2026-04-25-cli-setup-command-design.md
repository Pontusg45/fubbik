# CLI `fubbik setup` Command

**Date:** 2026-04-25
**Status:** Approved

## Problem

A solo developer with fubbik running wants to adopt it for their project. Today the path is fragmented: `init --scan` imports markdown docs, but there's no guided flow that understands the project's tech stack, structure, and conventions. The developer has to manually create chunks for everything beyond documentation.

## Solution

A new `fubbik setup` command that scans the project across three tiers of increasing inference depth, presents a summary for confirmation, imports everything in one shot, and finishes with tips about what it wasn't confident enough to auto-import.

## Persona & Scope

- **Who:** Solo developer, server already running, wants a populated knowledge base
- **Starting point:** `pnpm dev` is running, project directory has code
- **Success:** A well-structured knowledge base with docs, tech stack, project structure, and key conventions imported
- **Not in scope:** Team onboarding, web UI wizards, MCP/VS Code setup flows

## Command Shape

```
fubbik setup [options]

Options:
  --server <url>   Server URL (overrides config)
  --dry-run        Show preview without importing
  --yes            Skip confirmation prompt
  --force          Re-import even if chunks exist for this codebase
  --json           Output as JSON
  --quiet          Minimal output
```

## UX Flow

Six phases, sequential:

### Phase 0 — Preflight

- Resolve server URL (flag > config > error with hint)
- Test server connectivity (`GET /api/health`)
- Detect codebase via git remote (`GET /api/codebases/detect`)
- If no codebase found, create one (`POST /api/codebases`)
- If no git repo, prompt for a codebase name
- If codebase already has chunks and `--force` not set, warn and confirm

### Phase 1 — Discover

Run all three scanner tiers (see below). Collect `DiscoveryResult`.

### Phase 2 — Preview

Display a grouped summary table:

```
┌─────────────────────────────────────────────────┐
│ Ready to import 34 chunks                       │
├──────────────┬──────┬───────────────────────────┤
│ Category     │ Count│ Examples                  │
├──────────────┼──────┼───────────────────────────┤
│ Documents    │   12 │ README, API docs, guides  │
│ Tech stack   │    6 │ Elysia, Drizzle, Tailwind │
│ Structure    │    8 │ apps/web, packages/db, ... │
│ Conventions  │    5 │ Effect error handling, ... │
│ Config       │    3 │ TypeScript strict, ESM     │
├──────────────┴──────┴───────────────────────────┤
│ + 14 connections (dependency, part_of, ...)     │
│ + 23 tags                                       │
└─────────────────────────────────────────────────┘
```

If `--dry-run`: print preview and exit.

### Phase 3 — Confirm

Single prompt: `Import 34 chunks to fubbik? [Y/n]`

Skipped with `--yes`.

### Phase 4 — Import

- Tier 1 docs via `POST /api/chunks/import-docs` (batched, reuses existing endpoint)
- Tier 2/3 chunks via `POST /api/chunks` (parallel, concurrency 5)
- Connections via `POST /api/connections` (after all chunks exist, needs ID resolution)
- Progress output: `✓ 34 chunks created`, `✓ 14 connections established`
- Partial failures: continue, collect errors, show summary

### Phase 5 — Tips

Print lower-confidence detections as suggestions:

```
You might also want to add:
  • Testing patterns — vitest with describe/it, tests in __tests__/
  • Git workflow — conventional commits detected (feat:, fix:, ...)

View your knowledge graph: http://localhost:3001/graph
```

## Discovery Tiers

### Tier 1 — Documents (high confidence)

Reuses existing `scanner.ts` logic, extracted to `tier1-docs.ts`:

- Root docs: `README.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `CHANGELOG.md`
- `docs/` directory (recursive `.md`)
- Other `.md` files (max depth 5)
- Ignores: `node_modules`, `.git`, `.turbo`, `dist`, `build`, `.next`, `.output`, `.cache`, `coverage`, `.fubbik`
- Auto-split large files by H1-H3 headings (existing threshold logic)
- Types: `"document"` or `"guide"`, tags from folder path + frontmatter

### Tier 2 — Project Metadata (high confidence)

Reads config files to produce concise summary chunks. No code parsing.

| Source | Chunk produced |
|--------|---------------|
| `package.json` (root + workspaces) | Tech stack — framework, major deps, dev tooling. Detects monorepo from `workspaces` field. |
| `tsconfig.json` / `jsconfig.json` | TypeScript config — strict mode, path aliases, target |
| `.env.example` / `.env.local.example` | Environment variables — lists expected vars (never reads actual `.env`) |
| `docker-compose.yml` / `Dockerfile` | Infrastructure — services, ports, build stages |
| `turbo.json` / `nx.json` | Build pipeline — task graph, caching config |
| CI files (`.github/workflows/`, `.gitlab-ci.yml`) | CI/CD — what's tested, built, deployed |

Each config file maps to **at most one chunk**. Content is a human-readable summary of the facts, not a raw dump of the file.

- Type: `"reference"` for config descriptions, `"schema"` for env vars
- Tags: dependency categories (`"framework"`, `"database"`, `"testing"`, `"ci"`, `"infrastructure"`)

### Tier 3 — Code Patterns (conservative)

Filesystem traversal + targeted regex. No AST parsing. A pattern is only included if **both** the dependency exists in `package.json` **AND** the expected file/directory structure is found. Either signal alone goes to Tips.

| Detection | Signals required | Chunk |
|-----------|-----------------|-------|
| Route structure | Framework dep + `routes/`, `pages/`, or `api/` dirs with matching file patterns | "API routes at `X`, follow Y pattern" |
| Test patterns | Test runner dep + `*.test.ts`/`*.spec.ts`/`__tests__/` presence | "Tests use X, located at Y" |
| Database/ORM | ORM dep + `schema.ts`/`migrations/` dirs | "DB schema at X, uses Y" |
| Component structure | UI framework dep + `components/`/`features/` dirs | "Components organized by feature in X" |
| Auth | Auth library dep + `auth.ts`/auth config files | "Authentication via X at Y" |

- Type: `"convention"` for patterns, `"reference"` for structural descriptions
- Tags: pattern names (`"routing"`, `"auth"`, `"components"`, `"database"`, `"testing"`)
- `appliesTo`: glob patterns derived from detected paths (e.g., `"packages/api/src/*/routes.ts"`)

### Connections

Generated between discovered chunks:

| Condition | Relation |
|-----------|----------|
| Tier 1 docs referencing each other (markdown links) | `references` |
| Workspace packages → monorepo structure chunk | `part_of` |
| Tier 3 patterns with dependencies (routes → DB schema) | `depends_on` |
| Tier 2/3 chunk title keywords found in a tier 1 doc's content (case-insensitive substring match on dependency names, framework names, or directory paths) | `supports` |

### Tips (below confidence threshold)

Detections where only one signal was found:

- Dependency present but no matching file structure
- File structure present but no matching dependency
- Patterns detected but too ambiguous to summarize (e.g., custom architectural patterns)

Each tip has a `title` and one-line `detail` explaining what was detected.

## Architecture

### New files

```
apps/cli/src/
├── commands/
│   └── setup.ts              # Command definition, phase orchestration
└── lib/
    └── setup/
        ├── index.ts           # Re-exports
        ├── discover.ts        # Orchestrates tiers, returns DiscoveryResult
        ├── tier1-docs.ts      # Extracted from existing scanner.ts
        ├── tier2-metadata.ts  # Config file readers
        ├── tier3-patterns.ts  # Code pattern detection
        ├── preview.ts         # Summary table formatting
        ├── import.ts          # Server API calls
        └── tips.ts            # Lower-confidence suggestion generation
```

### Types

```typescript
interface DiscoveryResult {
  codebase: { name: string; remoteUrl: string | null; localPath: string };
  chunks: DiscoveredChunk[];
  connections: DiscoveredConnection[];
  tags: string[];
  tips: Tip[];
}

interface DiscoveredChunk {
  title: string;
  content: string;
  type: string;
  tags: string[];
  tier: 1 | 2 | 3;
  source: string;        // File path or detection name
  appliesTo?: string[];  // Glob patterns for file-area linking
}

interface DiscoveredConnection {
  sourceTitle: string;   // Resolved to IDs after import
  targetTitle: string;
  relation: string;
}

interface Tip {
  title: string;
  detail: string;
}
```

### Refactoring existing code

The doc-scanning logic in `scanner.ts` is extracted to `tier1-docs.ts` without behavior changes. Both `init --scan` and `setup` call the same function. `init` is not modified beyond importing from the new location.

### Server interaction

No new API endpoints. The command uses:

1. `GET /api/health` — connectivity check
2. `GET /api/codebases/detect` — find existing codebase
3. `POST /api/codebases` — create if not found
4. `POST /api/chunks/import-docs` — tier 1 document chunks (existing bulk endpoint)
5. `POST /api/chunks` — tier 2/3 chunks (parallel, concurrency 5)
6. `POST /api/connections` — discovered connections (after chunks exist, title→ID resolution)

### Error handling

- Server unreachable → `"Cannot connect to server at <url>. Is it running?"`, exit 1
- Partial import failures → continue, collect errors, print summary at end
- No git repo → skip remote detection, prompt for codebase name
- Empty project (nothing found across all tiers) → `"No knowledge sources found in this project."`, exit 0
- Codebase already has chunks (no `--force`) → `"This codebase already has N chunks. Run with --force to re-import."`, confirm prompt
