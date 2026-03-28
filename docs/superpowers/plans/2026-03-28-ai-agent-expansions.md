# AI Agent Expansions Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add tool chain recipes, context window optimizer, and agent feedback loop for better AI agent outcomes.

**Architecture:** Recipes are stored as chunks tagged "tool-recipe" with structured content. Context optimizer uses health scores + deduplication to maximize value per token. Feedback stores per-chunk usefulness ratings and feeds back into context scoring.

**Tech Stack:** Elysia, Effect, MCP, CLI

---

## Task 1: Tool Chain Recipes (#7)

Named sequences of MCP tool calls that agents can execute by name.

**Files:**
- Create: `apps/cli/src/commands/recipe.ts` — CLI for managing recipes
- Create: `packages/mcp/src/recipe-tools.ts` — MCP tools for recipes
- Modify: `packages/mcp/src/index.ts` — register recipe plugin

- [ ] **Step 1:** Recipes are chunks tagged "tool-recipe" with structured content:
```markdown
# Review Recipe

**Steps:**
1. get_conventions
2. search_chunks: { "query": "{{topic}}" }
3. begin_implementation: { "title": "Review: {{topic}}" }
4. complete_implementation
```

- [ ] **Step 2:** Create `recipe.ts` CLI:
- `fubbik recipe list` — list chunks tagged "tool-recipe"
- `fubbik recipe get <name>` — output recipe steps
- `fubbik recipe add <name> --content-file <file>` — create recipe chunk

- [ ] **Step 3:** Create MCP tools:
- `get_recipe` — fetch recipe by name, return the tool sequence
- `list_recipes` — list available recipes
Register as `recipePlugin`.

- [ ] **Step 4:** Commit.

---

## Task 2: Context Window Optimizer (#8)

Smarter context export that deduplicates and condenses.

**Files:**
- Create: `packages/api/src/context-export/optimizer.ts` — dedup + condense logic
- Modify: `packages/api/src/context-export/service.ts` — add `optimize` flag
- Modify: `packages/api/src/context-export/routes.ts` — add `optimize` query param
- Modify: `apps/cli/src/commands/context.ts` — add `--optimize` flag

- [ ] **Step 1:** Create optimizer that:
1. Groups chunks by topic (using tags/connections)
2. Detects content overlap (simple: shared sentences > 50%)
3. For overlapping chunks, keeps the higher-scored one
4. Trims redundant headers/footers from remaining chunks
5. Returns the optimized set with token savings reported

- [ ] **Step 2:** In the context export service, when `optimize: true`, run the optimizer before the token budget selection.

- [ ] **Step 3:** Add `optimize: t.Optional(t.String())` to the route. Add `--optimize` to the CLI context command.

- [ ] **Step 4:** Commit.

---

## Task 3: Agent Feedback Loop (#9)

Agents rate which chunks were useful after a session.

**Files:**
- Create: `packages/db/src/schema/chunk-rating.ts` — rating table
- Create: `packages/db/src/repository/chunk-rating.ts` — CRUD
- Create: `packages/api/src/chunk-ratings/routes.ts` — API
- Create: `packages/mcp/src/feedback-tools.ts` — MCP tools
- Modify: `packages/api/src/context-export/service.ts` — factor ratings into scoring

- [ ] **Step 1:** Create `chunk_rating` table:
```ts
{
    id: text PK,
    chunkId: text FK → chunk,
    sessionId: text FK → implementation_session (nullable),
    userId: text FK → user,
    rating: integer, // 1=irrelevant, 2=slightly useful, 3=useful, 4=very useful, 5=essential
    createdAt: timestamp
}
```
Export, push schema.

- [ ] **Step 2:** Create repo + routes:
- `POST /chunk-ratings` — rate a chunk
- `GET /chunk-ratings/stats?chunkId=` — average rating for a chunk
- `GET /chunk-ratings/top` — highest-rated chunks

- [ ] **Step 3:** Create MCP tools:
- `rate_chunk` — rate a chunk's usefulness after using it
- `get_top_rated_chunks` — get highest-rated chunks for context

- [ ] **Step 4:** In context export scoring, add a rating bonus: chunks with avg rating ≥ 4 get +5 score points. Import rating stats and boost high-rated chunks.

- [ ] **Step 5:** Commit.
