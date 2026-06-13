# Build a Full Fubbik Knowledge System from a Codebase

Orchestration skill for AI agents. Given a codebase, systematically analyze it, populate a fubbik knowledge base with chunks, connections,
tags, requirements, behavioral matrices, and enrichment — producing a navigable, AI-queryable knowledge graph.

## Prerequisites

Before starting, verify:

```
fubbik health                     # Server running at configured URL
fubbik codebase current           # Or detect via git remote
ollama list                       # nomic-embed-text + llama3.2 available
```

If `fubbik health` fails → the server is not running. Start it (`pnpm dev` in the fubbik project) before proceeding.

If `ollama list` shows missing models → `ollama pull nomic-embed-text && ollama pull llama3.2`. Enrichment and semantic search require
these.

If no codebase is detected → you'll create one in Phase 1.

---

## Orchestration Overview

```
Phase 1: Bootstrap          → init, detect/create codebase, establish identity
Phase 2: Automated Scan     → 3-tier discovery (docs, metadata, patterns)
Phase 3: Deep Analysis      → manual chunk creation for architecture, decisions, conventions
Phase 4: Connections        → typed edges between chunks (part_of, depends_on, etc.)
Phase 5: File Mapping       → appliesTo globs + fileReferences for code ↔ knowledge links
Phase 6: Requirements       → BDD-style specs for key behaviors
Phase 7: Behavioral Matrix  → invariant + contract matrices for system rules
Phase 8: Plans              → implementation plans linking chunks, requirements, tasks
Phase 9: Enrichment         → AI-generated summaries, aliases, embeddings
Phase 10: Validation        → health checks, gap analysis, graph review
Phase 11: Context Export    → CLAUDE.md generation, MCP readiness
```

Each phase has a **gate** — a verification step before proceeding. Phases 3-8 can be partially parallelized by domain area.

---

## Phase 1: Bootstrap

**Goal:** Initialize fubbik, detect or create the codebase entry.

### Steps

```bash
# 1. Initialize local store (skip if .fubbik/ already exists)
fubbik init --server http://localhost:3000

# 2. Detect codebase from git remote
fubbik codebase current --json
```

If no codebase is detected:

```bash
# Get the git remote URL
git remote get-url origin

# Create the codebase
curl -X POST http://localhost:3000/api/codebases \
  -H "Content-Type: application/json" \
  -d '{"name": "<project-name>", "remoteUrl": "<git-remote-url>", "localPaths": ["<absolute-path>"]}'
```

### Gate

- `fubbik health` returns OK
- `fubbik codebase current` returns a valid codebase ID
- Store the codebase ID — you'll use it in every subsequent phase

---

## Phase 2: Automated Scan (3-Tier Discovery)

**Goal:** Let fubbik's built-in scanner create an initial chunk population.

### Option A: Interactive Setup (Recommended)

```bash
fubbik setup --server http://localhost:3000
```

This runs the full 3-tier pipeline:

- **Tier 1 — Document scanning:** README.md, CLAUDE.md, CONTRIBUTING.md, docs/ directory, scattered .md files
- **Tier 2 — Metadata analysis:** package.json dependencies → tech stack chunks, monorepo structure, config files
- **Tier 3 — Code pattern detection:** repository/service/route patterns, frontend structure, test patterns

It previews results before importing and creates connections automatically.

### Option B: Manual Import

For more control, import in stages:

```bash
# Import all markdown docs
fubbik import docs/ --server --codebase <name> --recursive

# Import specific files
fubbik import README.md --server --codebase <name>
fubbik import CLAUDE.md --server --codebase <name>
fubbik import CONTRIBUTING.md --server --codebase <name>
```

### Gate

```bash
fubbik stats
# Expect: >0 chunks created, covering documentation
fubbik list --json | head -20
# Verify: titles are meaningful, types are correct
```

---

## Phase 3: Deep Analysis — Manual Chunk Creation

**Goal:** Create the chunks that automated scanning cannot produce — architecture decisions, conventions, domain concepts, and system
knowledge that lives in developers' heads.

### 3.1 Architecture Overview Chunk

Create a top-level architecture document that maps the system:

```bash
fubbik add \
  --type document \
  --title "Architecture Overview" \
  --content "$(cat <<'CONTENT'
## System Architecture

[Describe the high-level architecture: what are the major subsystems, how do they communicate, what are the deployment boundaries]

## Key Design Decisions

[List the 3-5 most important architectural choices and why they were made]

## Technology Choices

[Runtime, framework, database, auth, etc. — and WHY each was chosen]

## Data Flow

[How data moves through the system: request → route → service → repository → database]
CONTENT
)" \
  --tags architecture,overview \
  --codebase <name>
```

### 3.2 Domain Model Chunks

For each major domain entity, create a chunk:

```bash
fubbik add \
  --type schema \
  --title "<Entity> Domain Model" \
  --content "[Fields, relationships, invariants, lifecycle states]" \
  --tags domain-model,<entity-name> \
  --codebase <name>
```

**Heuristic:** Look at the database schema directory. Each table or logical group of tables is typically one domain entity worth
documenting.

### 3.3 Convention Chunks

Extract conventions from code patterns. For each distinct pattern:

```bash
fubbik add \
  --type note \
  --title "<Area>: <Convention Name>" \
  --content "[What the convention is, how to follow it, why it exists]" \
  --tags convention,<area> \
  --codebase <name>
```

**Where to find conventions:**

- Linter configs (eslint, prettier) → formatting conventions
- tsconfig.json → module resolution, path aliases
- Test files → testing conventions (describe/it structure, mocking approach)
- Error handling patterns → how errors flow through the system
- API route handlers → request/response patterns
- Component structure → UI composition patterns

### 3.4 Architecture Decision Records (ADRs)

For each significant decision (check git history, docs/adr/, CHANGELOG):

```bash
fubbik add \
  --type document \
  --title "ADR: <Decision Title>" \
  --content "[Context, Decision, Consequences]" \
  --tags adr,architecture \
  --codebase <name>
```

Use the `--rationale`, `--alternatives`, `--consequences` fields when available:

```bash
curl -X POST http://localhost:3000/api/chunks \
  -H "Content-Type: application/json" \
  -d '{
    "title": "ADR: Use Effect for Error Handling",
    "content": "We use the Effect library for typed, composable error handling in the service layer.",
    "type": "document",
    "tags": ["adr", "architecture", "error-handling"],
    "rationale": "Typed errors enable exhaustive handling at route boundaries. Untyped try/catch was causing silent failures.",
    "alternatives": "- Plain try/catch with custom error classes\n- neverthrow library\n- fp-ts Either",
    "consequences": "- All service functions return Effect<T, Error>\n- Route handlers use Effect.runPromise\n- Learning curve for Effect library",
    "codebaseIds": ["<codebase-id>"]
  }'
```

### 3.5 Runbook / How-To Chunks

For operational procedures (deployment, debugging, setup):

```bash
fubbik add \
  --type checklist \
  --title "Runbook: <Procedure>" \
  --content "[Step-by-step checklist with verification steps]" \
  --tags runbook,operations \
  --codebase <name>
```

### 3.6 API Reference Chunks

For each API surface area:

```bash
fubbik add \
  --type reference \
  --title "API: <Endpoint Group>" \
  --content "[Endpoints, methods, request/response schemas, auth requirements]" \
  --tags api,reference \
  --codebase <name>
```

### Chunk Creation Guidelines

| Rule                          | Detail                                                   |
| ----------------------------- | -------------------------------------------------------- |
| **One concept per chunk**     | Split if a chunk covers >1 topic                         |
| **50–300 lines**              | Target size for content                                  |
| **Descriptive titles**        | "PostgreSQL Connection Pooling" not "Database Notes"     |
| **3–6 tags**                  | Domain tags + technology tags + content-type tags        |
| **Lowercase hyphenated tags** | `error-handling` not `ErrorHandling`                     |
| **Check existing tags first** | `fubbik tags --json` before inventing new ones           |
| **No duplicates**             | Search before creating: `fubbik search "<title>" --json` |
| **Index by domain, not file** | Group by concept, not by filesystem path                 |

### Gate

```bash
fubbik list --json | jq 'length'
# Expect: 20-100+ chunks depending on codebase size

fubbik list --type document --json | jq 'length'
fubbik list --type reference --json | jq 'length'
fubbik list --type schema --json | jq 'length'
fubbik list --type note --json | jq 'length'
fubbik list --type checklist --json | jq 'length'
# Expect: distribution across types, not all "note" or all "document"
```

---

## Phase 4: Connections

**Goal:** Create typed directed edges between chunks to form a navigable knowledge graph.

### Connection Types

| Relation         | When to Use                               | Direction              |
| ---------------- | ----------------------------------------- | ---------------------- |
| `part_of`        | Hierarchical containment                  | child → parent         |
| `depends_on`     | Functional dependency (breaks if removed) | dependent → dependency |
| `references`     | Mentions or cites                         | referrer → referenced  |
| `extends`        | Builds upon / specializes                 | extension → base       |
| `supports`       | Provides evidence or backing              | evidence → claim       |
| `related_to`     | General association (use sparingly)       | either direction       |
| `contradicts`    | Conflicts with                            | either direction       |
| `alternative_to` | Different approach to same problem        | either direction       |

### Strategy

Build connections in layers:

**Layer 1 — Structural (part_of):**

```bash
# Architecture Overview is the root
fubbik link <auth-module-id> <architecture-overview-id> --relation part_of
fubbik link <api-layer-id> <architecture-overview-id> --relation part_of
fubbik link <database-id> <architecture-overview-id> --relation part_of

# Sub-components belong to their parent module
fubbik link <session-management-id> <auth-module-id> --relation part_of
fubbik link <oauth-flow-id> <auth-module-id> --relation part_of
```

**Layer 2 — Dependencies (depends_on):**

```bash
# API routes depend on auth, database, etc.
fubbik link <api-routes-id> <auth-module-id> --relation depends_on
fubbik link <api-routes-id> <database-schema-id> --relation depends_on
fubbik link <service-layer-id> <repository-layer-id> --relation depends_on
```

**Layer 3 — Cross-references (references):**

```bash
# ADRs reference the systems they affect
fubbik link <adr-effect-errors-id> <service-layer-id> --relation references
fubbik link <deployment-runbook-id> <env-config-id> --relation references
```

**Layer 4 — Semantic (related_to, supports, extends):**

```bash
# Convention supports the architecture it describes
fubbik link <naming-convention-id> <code-style-id> --relation supports
```

### Bulk Connection via API

For many connections, use the API directly:

```bash
# Create connection
curl -X POST http://localhost:3000/api/connections \
  -H "Content-Type: application/json" \
  -d '{"sourceId": "<child-id>", "targetId": "<parent-id>", "relation": "part_of"}'
```

### Gate

```bash
fubbik stats
# Check connection count — aim for at least 1.5x the chunk count

# Verify in graph view
fubbik open graph
# Look for: no isolated nodes, clear clustering, readable relation labels
```

---

## Phase 5: File Mapping

**Goal:** Link chunks to the source files they describe, enabling `fubbik context-for <file>` and `fubbik check-files` to work.

### 5.1 AppliesTo Patterns (Glob-Based)

Associate chunks with file patterns:

```bash
# Via API (preferred for bulk)
curl -X PUT http://localhost:3000/api/chunks/<chunk-id>/applies-to \
  -H "Content-Type: application/json" \
  -d '[
    {"pattern": "packages/api/src/**/*.ts"},
    {"pattern": "packages/db/src/repository/**"}
  ]'
```

**Pattern guidelines:**

- Use glob syntax: `**/*.ts`, `src/features/auth/**`, `packages/db/src/schema/*.ts`
- Be specific enough to avoid false matches
- One chunk can have multiple patterns
- Test with: `fubbik context-for <file-path> --json`

### 5.2 File References (Explicit Links)

For precise file-to-chunk linking:

```bash
curl -X PUT http://localhost:3000/api/chunks/<chunk-id>/file-refs \
  -H "Content-Type: application/json" \
  -d '[
    {"path": "packages/api/src/chunks/routes.ts", "symbol": "chunkRoutes"},
    {"path": "packages/db/src/schema/chunks.ts"}
  ]'
```

### Mapping Strategy

| Chunk Type        | Mapping Approach                                      |
| ----------------- | ----------------------------------------------------- |
| Architecture docs | Broad globs: `packages/<module>/**`                   |
| Conventions       | Narrow globs: `**/*.test.ts` for testing conventions  |
| API reference     | Specific files: `packages/api/src/<domain>/routes.ts` |
| Schema docs       | Exact files: `packages/db/src/schema/<table>.ts`      |
| Runbooks          | Config files: `docker-compose.yml`, `Caddyfile`       |

### Gate

```bash
# Test file context resolution for key files
fubbik context-for src/index.ts --json
fubbik context-for packages/api/src/chunks/routes.ts --json

# Check for unmapped source files
fubbik gaps src/ --limit 50
fubbik gaps packages/ --limit 50
# Target: <20% of source files ungapped
```

---

## Phase 6: Requirements

**Goal:** Define BDD-style requirements for key system behaviors. Requirements link to chunks and plans, providing traceability.

### Creating Requirements

```bash
fubbik req create \
  --title "User can search chunks by text query" \
  --priority must \
  --steps '[
    {"keyword": "Given", "text": "chunks exist in the knowledge base"},
    {"keyword": "When", "text": "the user searches with a text query"},
    {"keyword": "Then", "text": "matching chunks are returned sorted by relevance"},
    {"keyword": "And", "text": "the search supports partial word matching"}
  ]'
```

Or via API:

```bash
curl -X POST http://localhost:3000/api/requirements \
  -H "Content-Type: application/json" \
  -d '{
    "title": "User can search chunks by text query",
    "description": "Full-text search across chunk titles and content",
    "priority": "must",
    "status": "untested",
    "steps": [
      {"keyword": "Given", "text": "chunks exist in the knowledge base"},
      {"keyword": "When", "text": "the user searches with a text query"},
      {"keyword": "Then", "text": "matching chunks are returned sorted by relevance"}
    ],
    "codebaseId": "<codebase-id>"
  }'
```

### Requirement Categories to Cover

Work through these systematically for the target codebase:

1. **Core CRUD Operations** — Can the user create, read, update, delete the primary entities?
2. **Search & Discovery** — Can the user find what they need via text, semantic, filtered search?
3. **Data Integrity** — Are invariants maintained? (unique constraints, cascading deletes, etc.)
4. **Authentication & Authorization** — Who can do what? Are boundaries enforced?
5. **API Contracts** — Do endpoints return the documented schemas? Error codes correct?
6. **Performance** — Are there latency/throughput expectations?
7. **Integration Points** — Do external integrations (Ollama, git, MCP) handle failures gracefully?

### Linking Requirements to Chunks

```bash
# Link a requirement to the chunks it verifies
curl -X POST http://localhost:3000/api/requirements/<req-id>/chunks \
  -H "Content-Type: application/json" \
  -d '{"chunkId": "<chunk-id>"}'
```

### Priority Levels

| Priority | Meaning                                          |
| -------- | ------------------------------------------------ |
| `must`   | System is broken without this                    |
| `should` | Expected behavior, important                     |
| `could`  | Nice to have, enhances experience                |
| `wont`   | Explicitly out of scope (documented for clarity) |

### Gate

```bash
fubbik req list --json | jq 'length'
# Expect: 10-50+ requirements for a meaningful codebase

fubbik req stats
# Check: distribution across priorities

curl http://localhost:3000/api/requirements/coverage
# Check: requirements are linked to chunks
```

---

## Phase 7: Behavioral Matrices

**Goal:** Create structured specification matrices that map rules against dimensions — either entity invariants or actor capabilities.

### 7.1 Invariant Matrix

Maps data rules against entities:

```bash
# Create the matrix
fubbik matrix create "Data Invariants" --layer invariant \
  --description "Rules that must hold true for each entity"

# Add dimensions (columns) — one per entity
fubbik matrix add-dimension <matrix-id> "Chunk"
fubbik matrix add-dimension <matrix-id> "Connection"
fubbik matrix add-dimension <matrix-id> "Tag"
fubbik matrix add-dimension <matrix-id> "Requirement"
fubbik matrix add-dimension <matrix-id> "Plan"

# Add rules (rows) — one per invariant
fubbik matrix add-rule <matrix-id> "Must have non-empty title" --category "validation"
fubbik matrix add-rule <matrix-id> "Cascade delete removes dependents" --category "lifecycle"
fubbik matrix add-rule <matrix-id> "Unique within scope" --category "constraint"
fubbik matrix add-rule <matrix-id> "Version history preserved on update" --category "audit"

# Mark which cells are specified (rule applies to entity)
fubbik matrix cell <matrix-id> <rule-id> <dimension-id>
# Repeat for each applicable cell
```

### 7.2 Contract Matrix

Maps capabilities against actors:

```bash
fubbik matrix create "API Capabilities" --layer contract \
  --description "What each actor can do via the API"

# Dimensions = actors
fubbik matrix add-dimension <matrix-id> "Authenticated User"
fubbik matrix add-dimension <matrix-id> "Anonymous User"
fubbik matrix add-dimension <matrix-id> "Admin"
fubbik matrix add-dimension <matrix-id> "MCP Agent"
fubbik matrix add-dimension <matrix-id> "CLI User"

# Rules = capabilities
fubbik matrix add-rule <matrix-id> "Create chunks" --category "chunks"
fubbik matrix add-rule <matrix-id> "Delete chunks" --category "chunks"
fubbik matrix add-rule <matrix-id> "Search semantic" --category "search"
fubbik matrix add-rule <matrix-id> "Export context" --category "context"
fubbik matrix add-rule <matrix-id> "Manage codebases" --category "admin"

# Toggle cells
fubbik matrix cell <matrix-id> <rule-id> <dimension-id>
```

### 7.3 Link Requirements to Matrix Cells

For each matrix cell that maps to a testable behavior, link a requirement:

```bash
fubbik matrix link <cell-id> <requirement-id> --matrix <matrix-id>
```

### 7.4 Find Gaps

```bash
fubbik matrix gaps <matrix-id>
# Shows: unspecified cells (no status) and cells without linked requirements
```

### Gate

```bash
fubbik matrix list --json
# Expect: at least 1 invariant + 1 contract matrix

fubbik matrix show <matrix-id> --json
# Check: cells are populated, not all empty

fubbik matrix gaps <matrix-id>
# Target: <30% unspecified cells
```

---

## Phase 8: Plans

**Goal:** Create implementation plans that link chunks to tasks, providing a roadmap for work.

### Creating a Plan

```bash
fubbik plan create "Implement Authentication Flow" \
  --description "Set up Better Auth with email/password and OAuth providers" \
  --codebase <name>
```

Or via API with full structure:

```bash
curl -X POST http://localhost:3000/api/plans \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Implement Authentication Flow",
    "description": "Set up Better Auth with email/password and OAuth providers",
    "codebaseId": "<codebase-id>",
    "requirementIds": ["<req-1>", "<req-2>"],
    "tasks": [
      {
        "title": "Configure Better Auth server",
        "description": "Set up auth.ts with database adapter and session config",
        "acceptanceCriteria": ["Sessions persist across restarts", "Password hashing uses bcrypt"]
      },
      {
        "title": "Add login/signup routes",
        "description": "Create Elysia routes for email/password auth",
        "acceptanceCriteria": ["POST /api/auth/sign-up creates user", "POST /api/auth/sign-in returns session"]
      }
    ]
  }'
```

### Adding Analyze Items

Plans support structured analysis:

```bash
# Add a risk
curl -X POST http://localhost:3000/api/plans/<plan-id>/analyze \
  -H "Content-Type: application/json" \
  -d '{"kind": "risk", "title": "Session token storage may not meet compliance", "severity": "high"}'

# Add an assumption
curl -X POST http://localhost:3000/api/plans/<plan-id>/analyze \
  -H "Content-Type: application/json" \
  -d '{"kind": "assumption", "title": "Ollama will be available locally", "verified": false}'

# Link a chunk for context
curl -X POST http://localhost:3000/api/plans/<plan-id>/analyze \
  -H "Content-Type: application/json" \
  -d '{"kind": "chunk", "chunkId": "<chunk-id>"}'

# Link a file
curl -X POST http://localhost:3000/api/plans/<plan-id>/analyze \
  -H "Content-Type: application/json" \
  -d '{"kind": "file", "title": "packages/auth/src/auth.ts", "lineStart": 1, "lineEnd": 50}'
```

### Linking Tasks to Chunks

```bash
curl -X POST http://localhost:3000/api/plans/<plan-id>/tasks/<task-id>/chunks \
  -H "Content-Type: application/json" \
  -d '{"chunkId": "<chunk-id>", "relation": "context"}'
# Relations: context, created, modified
```

### Gate

```bash
fubbik plan list --json | jq 'length'
# Expect: 1+ plans with tasks

fubbik plan show <plan-id> --json
# Check: tasks have acceptance criteria, requirements are linked
```

---

## Phase 9: Enrichment

**Goal:** Run AI enrichment to generate summaries, aliases, not-about terms, and embeddings for all chunks.

### Prerequisites Check

```bash
# Verify Ollama is running with required models
curl http://localhost:11434/api/tags | jq '.models[].name'
# Must include: nomic-embed-text, llama3.2
```

### Run Enrichment

```bash
# Enrich all chunks (runs at concurrency 3)
fubbik enrich --all
```

Or selectively:

```bash
# Enrich a single chunk
fubbik enrich <chunk-id>

# Enrich via API
curl -X POST http://localhost:3000/api/chunks/<chunk-id>/enrich
```

### What Enrichment Produces

For each chunk:

- **`summary`** — 1-2 sentence TL;DR (used in search results, graph tooltips)
- **`aliases`** — alternative names for better search recall ("Auth" → ["authentication", "login", "sign-in"])
- **`notAbout`** — terms that sound related but aren't (reduces false positives)
- **`embedding`** — 768-dim vector (nomic-embed-text) for semantic similarity search

### Gate

```bash
# Check enrichment coverage
curl http://localhost:3000/api/health/knowledge | jq '.staleEmbeddings'
# Target: 0 stale embeddings

# Test semantic search
fubbik search "how to deploy" --semantic --json
# Should return relevant results ranked by similarity
```

---

## Phase 10: Validation

**Goal:** Verify the knowledge system is complete, well-connected, and healthy.

### 10.1 Knowledge Health Check

```bash
curl http://localhost:3000/api/health/knowledge | jq
```

This returns:

- **orphanChunks** — chunks with zero connections (should be <10% of total)
- **staleChunks** — chunks not updated in 90+ days
- **thinChunks** — chunks with very short content (<50 chars)
- **staleEmbeddings** — chunks whose content changed after embedding generation
- **fileRefsBroken** — file references pointing to non-existent files

### 10.2 Gap Analysis

```bash
# Find source files with no associated knowledge
fubbik gaps . --limit 100

# Check specific directories
fubbik gaps src/features/ --limit 50
fubbik gaps packages/api/src/ --limit 50
```

**Target:** <20% of meaningful source files should be ungapped. Not every file needs a chunk — utility files, generated files, and simple
re-exports can be skipped.

### 10.3 Stats Review

```bash
fubbik stats
```

**Healthy knowledge base indicators:**

- Chunks: 20-200+ (scales with codebase size)
- Connections: 1.5-3x chunk count
- Tags: 15-50 unique tags
- Type distribution: not all one type
- Average chunk size: 100-500 lines

### 10.4 Graph Visual Inspection

```bash
fubbik open graph
```

Look for:

- **No isolated nodes** — every chunk should have at least one connection
- **Clear clusters** — related chunks group together
- **Hub nodes** — architecture overview, key domain models should be central
- **No star topology** — if one chunk connects to everything, it's probably too broad

### 10.5 Staleness Scan

```bash
# Run age-based staleness detection
curl -X POST http://localhost:3000/api/chunks/stale/scan-age \
  -H "Content-Type: application/json" \
  -d '{"codebaseId": "<codebase-id>"}'

# Check results
curl "http://localhost:3000/api/chunks/stale?codebaseId=<codebase-id>" | jq
```

### 10.6 Lint

```bash
fubbik lint
```

Checks for common issues: missing tags, empty content, broken references, naming inconsistencies.

### Gate

All of the following should pass:

- [ ] Knowledge health: orphans <10%, no thin chunks, no stale embeddings
- [ ] Gap coverage: <20% ungapped source files
- [ ] Graph: no isolated nodes, clear clustering
- [ ] Stats: healthy distribution across types and tags
- [ ] Lint: no critical issues

---

## Phase 11: Context Export

**Goal:** Make the knowledge system consumable by AI agents and developers.

### 11.1 CLAUDE.md Generation

```bash
# One-time generation
fubbik sync-claude-md

# With specific tag filter
fubbik sync-claude-md --tag claude-context

# With token budget
fubbik sync-claude-md --max-tokens 32000

# Watch mode (re-generates on change)
fubbik sync-claude-md --watch --interval 30
```

This generates a structured `.claude/CLAUDE.md` containing:

- Chunks organized by type (conventions, architecture, references)
- Active requirements (sorted: failing → untested → passing)
- In-progress plans with task breakdown

### 11.2 File-Scoped Context

Verify file context works for key entry points:

```bash
# Test for a few representative files
fubbik context-for src/index.ts --json
fubbik context-for packages/api/src/chunks/routes.ts --json
fubbik context-for apps/web/src/features/auth/login.tsx --json
```

### 11.3 Directory Context

```bash
# Generate context for a directory
fubbik context-dir packages/api/src/
```

### 11.4 MCP Server Readiness

If the codebase will be consumed via MCP:

```bash
# Verify MCP tools work
fubbik mcp-tools

# Test key operations
# (These are the tools AI agents will use)
# search_chunks, get_chunk, get_context_for_file, list_plans, list_requirements
```

### 11.5 Context Snapshot (Optional)

For reproducible AI sessions:

```bash
curl -X POST http://localhost:3000/api/context/snapshot \
  -H "Content-Type: application/json" \
  -d '{"codebaseId": "<codebase-id>", "name": "Initial knowledge base snapshot"}'
```

### Gate

- [ ] `fubbik sync-claude-md` generates valid output
- [ ] `fubbik context-for <file>` returns relevant chunks for 3+ key files
- [ ] MCP tools return data (if using MCP)

---

## Parallelization Guide

For an orchestrator dispatching sub-agents, these phases can run concurrently:

```
Sequential (must complete in order):
  Phase 1 → Phase 2 → Phase 9

Parallelizable after Phase 2 completes:
  Phase 3 (chunks)    ← can split by domain area
  Phase 5 (file maps) ← can split by package/directory
  Phase 6 (requirements) ← independent of chunk order

Sequential after Phase 3:
  Phase 4 (connections) ← needs chunk IDs from Phase 3

Parallelizable after Phase 6:
  Phase 7 (matrices) ← needs requirement IDs
  Phase 8 (plans)    ← needs requirement + chunk IDs

Sequential at end:
  Phase 9 (enrichment) ← needs all chunks created
  Phase 10 (validation) ← needs enrichment complete
  Phase 11 (export)     ← needs validation passing
```

### Domain-Based Splitting

For large codebases, split Phase 3 by domain area and run sub-agents in parallel:

```
Sub-agent A: Authentication & Authorization chunks
Sub-agent B: API & Routing chunks
Sub-agent C: Database & Schema chunks
Sub-agent D: Frontend & UI chunks
Sub-agent E: Infrastructure & DevOps chunks
```

Each sub-agent creates chunks for its domain, then a coordinator runs Phase 4 (connections) across all domains.

---

## Chunk Type Decision Tree

Use this to decide the right type for each piece of knowledge:

```
Is it step-by-step instructions?
  → checklist (runbook, deploy guide, review process)

Is it a data model or type definition?
  → schema (database tables, API schemas, TypeScript interfaces)

Is it API documentation or a lookup table?
  → reference (endpoints, error codes, config options)

Is it a how-to or tutorial?
  → guide (setup guide, migration guide, integration walkthrough)

Is it long-form structured documentation?
  → document (architecture overview, ADR, design doc, spec)

Is it a short convention, observation, or decision?
  → note (coding convention, gotcha, tip, pattern)
```

---

## Tag Taxonomy Template

Start with these tag categories and expand as needed:

### Domain Tags

`auth`, `database`, `api`, `frontend`, `backend`, `infra`, `testing`, `deployment`, `monitoring`, `security`

### Technology Tags

Named after the specific technology: `postgres`, `react`, `elysia`, `drizzle`, `ollama`, `docker`, `caddy`

### Content Tags

`architecture`, `convention`, `configuration`, `troubleshooting`, `api-reference`, `data-model`, `runbook`, `adr`

### Lifecycle Tags

`stable`, `experimental`, `deprecated`, `draft`

### Special Tags

`claude-context` — marks chunks for inclusion in CLAUDE.md generation `overview` — marks top-level summary chunks

---

## Anti-Patterns

| Don't                                         | Do Instead                                                    |
| --------------------------------------------- | ------------------------------------------------------------- |
| Create one giant chunk per module             | Split into focused chunks (50-300 lines each)                 |
| Use "note" type for everything                | Pick the most specific type                                   |
| Skip connections                              | Aim for 1.5x connections per chunk                            |
| Duplicate content across chunks               | Create one chunk and connect it                               |
| Use file paths as titles                      | Use descriptive domain-oriented titles                        |
| Create chunks without tags                    | Always tag with 3-6 relevant tags                             |
| Import without enriching                      | Run `fubbik enrich --all` after every import batch            |
| Map every utility file                        | Focus on meaningful source files, skip generated/trivial code |
| Create requirements without linking to chunks | Every requirement should reference 1+ chunks                  |
| Build matrices without linked requirements    | Cells without requirements are undocumented                   |

---

## Ongoing Maintenance Checklist

After the initial build, keep the knowledge system alive:

```bash
# Weekly: check for staleness
fubbik recap --since 7d
curl -X POST http://localhost:3000/api/chunks/stale/scan-age -H "Content-Type: application/json"

# After major code changes: re-scan gaps
fubbik gaps src/ --limit 50

# After adding new chunks: re-enrich
fubbik enrich --all

# After schema changes: update domain model chunks
fubbik search "domain model" --json

# Continuously: keep CLAUDE.md fresh
fubbik sync-claude-md --watch
```

---

## Quick Reference: All Commands Used

| Command                                                         | Purpose                                |
| --------------------------------------------------------------- | -------------------------------------- |
| `fubbik init --server <url>`                                    | Initialize local store + server config |
| `fubbik setup --server <url>`                                   | Interactive 3-tier scan + import       |
| `fubbik health`                                                 | Server health check                    |
| `fubbik codebase current`                                       | Detect current codebase                |
| `fubbik add --type <t> --title <t> --content <c> --tags <tags>` | Create a chunk                         |
| `fubbik import <path> --server --codebase <name>`               | Import markdown/JSON                   |
| `fubbik link <src> <tgt> --relation <rel>`                      | Create connection                      |
| `fubbik search "<query>" --json`                                | Text search                            |
| `fubbik search "<query>" --semantic --json`                     | Semantic search                        |
| `fubbik list --type <t> --json`                                 | List chunks by type                    |
| `fubbik tags --json`                                            | List all tags                          |
| `fubbik gaps <dir>`                                             | Find files without knowledge           |
| `fubbik enrich --all`                                           | AI enrichment                          |
| `fubbik stats`                                                  | Knowledge base statistics              |
| `fubbik lint`                                                   | Check for issues                       |
| `fubbik recap --since <period>`                                 | Recent changes                         |
| `fubbik context-for <file> --json`                              | File-scoped context                    |
| `fubbik sync-claude-md`                                         | Generate CLAUDE.md                     |
| `fubbik req create --title <t> --priority <p> --steps <json>`   | Create requirement                     |
| `fubbik req list --json`                                        | List requirements                      |
| `fubbik matrix create <name> --layer <l>`                       | Create behavioral matrix               |
| `fubbik matrix add-dimension <id> <name>`                       | Add matrix column                      |
| `fubbik matrix add-rule <id> <title>`                           | Add matrix row                         |
| `fubbik matrix cell <id> <rule> <dim>`                          | Toggle matrix cell                     |
| `fubbik matrix gaps <id>`                                       | Find unspecified cells                 |
| `fubbik plan create <title>`                                    | Create a plan                          |
| `fubbik plan show <id> --json`                                  | Show plan detail                       |
| `fubbik open graph`                                             | Open graph visualization               |

---

## Completion Criteria

The knowledge system is considered fully built when:

1. **Coverage:** >80% of meaningful source files have associated chunks
2. **Connectivity:** Connection-to-chunk ratio is >1.5
3. **Enrichment:** All chunks have summaries and embeddings (0 stale)
4. **Requirements:** Key behaviors documented with BDD steps
5. **Matrices:** At least 1 invariant + 1 contract matrix with <30% unspecified cells
6. **Plans:** Active work tracked via plans with linked tasks
7. **Context:** `fubbik context-for <file>` returns useful results for any key file
8. **Export:** CLAUDE.md generates successfully with structured content
9. **Health:** Orphan chunks <10%, no thin chunks, lint passes
10. **Graph:** Visual inspection shows clear clustering with no isolated nodes
