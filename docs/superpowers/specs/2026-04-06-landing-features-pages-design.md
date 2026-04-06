# Landing Page Redesign & Features Page

## Problem

The current landing page (463 lines) has good content but lacks the sharp developer-tool aesthetic that would resonate with the open-source community discovering Fubbik. There is no dedicated features page — all conceptual content is crammed into the single landing route, making it hard to both give a quick overview and explain the mental model in depth.

## Approach

- **Developer tool aesthetic**: Clean, minimal, code-forward. Think Linear/Raycast. Terminal snippets, sharp typography, dark theme emphasis.
- **Two-page strategy**: Landing page is tight and punchy (hook + demo + install). Features page explains the mental model (Chunks → Connections → Context → Health) then groups features by concept.

## Design

### Landing Page (`/`)

#### Hero with Knowledge Graph Background

Replace the current abstract constellation canvas with an evolved version that feels like a living knowledge graph:

- Nodes rendered as small rounded rectangles with faint labels (e.g., "Convention", "Architecture", "API Endpoint", "Runbook")
- Connected by edges with subtle relation labels (depends_on, part_of, extends)
- Mouse interaction: nearby nodes glow, connections between them highlight
- Nodes drift slowly; edges have slight animated dashes or flow
- Reduced-motion: static snapshot, no animation

Content overlay:
- Bold tagline: "Structured knowledge for your codebase"
- One-line value prop: "Store, connect, and evolve what your team knows — where your code lives."
- Two CTAs: "Get Started" (primary, scrolls to install section) and "How it works" (secondary, links to `/features`)

#### Terminal Demo

Animated terminal widget showing a realistic workflow:

```
$ fubbik quick "Always use Effect for typed errors"
Created a8f3 — note

$ fubbik search "error handling"
3 chunks found across 2 codebases

$ fubbik context --for src/api/auth.ts
Found 5 relevant chunks (2,400 tokens)
```

- Typewriter animation with realistic delays between commands and output
- Auto-plays on scroll into view, loops after pause
- Static fallback for `prefers-reduced-motion`
- Dark background, monospace font, syntax-colored output (green for success, muted for labels)

#### Integration Grid

"Works where you work" heading with `text-xs uppercase tracking-wider text-muted-foreground` styling.

6 cards in a 3x2 (md) / 2x3 (sm) / 1-col (mobile) grid:

| Card | Icon | Description |
|------|------|-------------|
| CLI | Terminal | Full-featured command line with quick-add and context export |
| Web UI | LayoutDashboard | Dashboard, graph visualization, chunk editor |
| VS Code | Code2 | File-aware chunk browsing and inline editing |
| MCP Server | Bot | AI agent integration with implementation tracking |
| API | FileCode | REST endpoints with Eden treaty type-safe client |
| Semantic Search | Search | Ollama-powered vector search across codebases |

Styling: bordered cards, subtle hover (`bg-muted/40`), icon + name + one-line description.

#### Feature Teasers

3-4 horizontal cards that link to anchored sections on `/features`:

1. **Knowledge Graph** — "Visualize how your knowledge connects" → `/features#connections`
2. **Health Monitoring** — "Know when knowledge goes stale" → `/features#health`
3. **AI-Native Context** — "Right knowledge at the right time" → `/features#context`
4. **Requirements & Plans** — "BDD specs with implementation tracking" → `/features#surfaces`

Each card: bold title, one sentence, arrow icon on hover. No screenshots.

#### Get Started

Three install paths in a tab component:

**Tab 1: Docker (coming soon)**
```bash
git clone https://github.com/your-org/fubbik.git
cd fubbik
docker compose up
```

**Tab 2: Local**
```bash
git clone https://github.com/your-org/fubbik.git
cd fubbik
pnpm install
pnpm seed    # sample data
pnpm dev     # localhost:3001
```

**Tab 3: npm (coming soon)**
```bash
npx create-fubbik my-knowledge-base
cd my-knowledge-base
pnpm dev
```

Each tab: copyable code blocks with copy button. "Coming soon" badge on npm tab.

#### Footer

Minimal single row: Fubbik logo (left), "Built with TanStack, Elysia, Drizzle, Effect" (center), GitHub icon link (right).

---

### Features Page (`/features`)

#### Header

- Title: "How Fubbik Works"
- Subtitle: "The mental model, then the tools"

#### The Model (anchor: `#model`)

Horizontal flow diagram rendered with HTML/CSS (not an image):

```
[Chunks] → [Connections] → [Context] → [Action]
 Atomic     Typed edges     Smart        Where you
 units      between them    retrieval    use it
```

Each node is a card with an icon, name, and one-line description. Arrows between cards. On mobile, stacks vertically. This diagram is the conceptual anchor — every section below references back to it.

#### Concept Section 1: Chunks (anchor: `#chunks`)

Heading: "Atomic units of knowledge"

Explanation (2-3 sentences): What a chunk is and why atomic units beat monolithic docs. Each chunk is typed, tagged, scoped to codebases, and linked to files.

**Example chunk** rendered as a styled card:
- Title: "Always use Effect for typed errors"
- Type badge: "Convention"
- Tags: `#backend`, `#error-handling`
- Content preview: "Use Effect.tryPromise with tagged error types. The global error handler maps _tag to HTTP status codes..."
- Health score: 82/100 with colored bar
- Metadata: appliesTo `packages/api/**`, 3 connections

**Feature list** (compact, no cards):
- 5 chunk types: note, document, reference, schema, checklist
- Tags with typed categories
- Scope (JSONB metadata) and appliesTo (glob patterns)
- File references linking chunks to code
- Decision context: rationale, alternatives, consequences
- Version history (append-only)
- Templates (built-in + custom)
- AI enrichment: summary, aliases, notAbout

#### Concept Section 2: Connections (anchor: `#connections`)

Heading: "Typed relationships between knowledge"

Explanation: Connections are directed edges with semantic meaning. They're global (not codebase-scoped), enabling cross-project knowledge linking.

**Mini graph** rendered with CSS (5-6 nodes, 6-8 edges with labeled relation types). Shows a realistic example:
- "Auth Middleware" —depends_on→ "Session Token Format"
- "Auth Middleware" —part_of→ "Authentication System"
- "JWT Tokens" —contradicts→ "Session Cookies"
- "OAuth2 Flow" —extends→ "Auth Middleware"

Relation type badges shown below: depends_on, part_of, extends, references, supports, contradicts, alternative_to, related_to.

**Feature list:**
- Dependency tree view (incoming/outgoing grouped by type)
- Related chunk suggestions (embedding similarity)
- Connection creation with relation picker
- Cross-codebase connections
- Graph visualization with focus mode and filter presets

#### Concept Section 3: Context (anchor: `#context`)

Heading: "The right knowledge at the right time"

Explanation: Fubbik doesn't just store knowledge — it delivers it where you need it. File-aware context matching, token-budgeted exports, and AI agent integration.

**Visual: context flow diagram**
```
File path (src/api/auth.ts)
    ↓ file refs + glob matching + dependency analysis
Relevant chunks (5 found, ranked by relevance)
    ↓ token budgeting (fit within 4k tokens)
Context export (CLAUDE.md, MCP tool response, CLI output)
```

**Feature list:**
- File-aware context: file references, appliesTo globs, dependency matching
- Semantic search: Ollama embeddings, cosine similarity
- Federated search: cross-codebase queries
- Token-budgeted export: greedy fill with health/relevance scoring
- CLAUDE.md generation: auto-generate from tagged chunks
- MCP server: 15+ tools for AI agents
- Context-for-file API: chunks relevant to any file path

#### Concept Section 4: Health (anchor: `#health`)

Heading: "Knowledge that maintains itself"

Explanation: Knowledge rots silently. Fubbik detects staleness, flags duplicates, scores health, and surfaces what needs attention — before you discover outdated docs in production.

**Visual: health score breakdown card**
- Freshness: 20/25 (updated 12 days ago)
- Completeness: 25/25 (has rationale, alternatives, consequences)
- Richness: 18/25 (has summary, missing embedding)
- Connectivity: 25/25 (4 connections)
- Total: 88/100

Plus a staleness banner example: "Files linked to this chunk changed 3 days ago: src/auth/middleware.ts"

**Feature list:**
- Health scores: freshness + completeness + richness + connectivity (0-100)
- Staleness detection: age-based (90-day), file-change, duplicate divergence
- Dashboard "Attention Needed" widget with grouped flags
- Nav badge showing stale count
- Chunk detail banners with dismiss/suppress actions
- Knowledge health page: orphans, stale, thin, missing embeddings

#### Surfaces (anchor: `#surfaces`)

Heading: "Where you interact"

Grid of 5 integration points, more detailed than landing page:

**Web UI**
- Dashboard with stats, favorites, activity, attention widget
- Knowledge graph with focus mode, filter presets, path finding
- Chunk editor with templates, autosave, duplicate detection
- Requirements with BDD steps, plans with interactive checklists

**CLI**
- `fubbik quick` for instant capture
- `fubbik search` / `fubbik context` for retrieval
- `fubbik plan` for implementation tracking
- `fubbik sync-claude-md` for AI context generation

**VS Code**
- Sidebar with type/tag/sort filtering
- File-aware chunk surfacing in active editor
- Inline editing, quick-add, status bar

**MCP Server**
- 15+ tools for AI agents
- Implementation sessions with review briefs
- Plan creation and step tracking
- Context retrieval and CLAUDE.md sync

**API**
- REST endpoints with Swagger/OpenAPI docs
- Eden treaty for type-safe client
- Effect-based error handling with tagged types

#### Bottom CTA

"Ready to try it?" heading with the same tabbed install paths as the landing page.

## Out of Scope

- Blog or changelog page
- User testimonials or social proof
- Pricing (it's open source)
- Video demos or embedded recordings
- i18n / translations
