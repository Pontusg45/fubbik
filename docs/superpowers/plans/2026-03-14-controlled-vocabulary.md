# Controlled Vocabulary Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a per-codebase controlled vocabulary with typed slot expectations, a greedy longest-match parser, AI-suggested seeding, and real-time step validation in the requirements UI.

**Architecture:** `vocabulary_entry` table with word/category/expects. Parser tokenizes step text left-to-right with longest match, validates slot expectations. AI suggestion via Ollama. Parser integrated into requirements create/update flow as non-blocking warnings.

**Tech Stack:** Drizzle ORM, Effect, Elysia, TanStack Router/Query, Ollama (llama3.2), Vitest

**Spec:** `docs/superpowers/specs/2026-03-14-controlled-vocabulary-design.md`

---

## File Structure

### New files
- `packages/db/src/schema/vocabulary.ts` — vocabulary_entry table
- `packages/db/src/repository/vocabulary.ts` — CRUD + list by codebase
- `packages/db/src/__tests__/vocabulary.test.ts` — schema test
- `packages/api/src/vocabulary/parser.ts` — tokenizer + slot validator
- `packages/api/src/vocabulary/parser.test.ts` — parser tests
- `packages/api/src/vocabulary/suggest.ts` — AI suggestion via Ollama
- `packages/api/src/vocabulary/service.ts` — composes CRUD + parsing + suggestion
- `packages/api/src/vocabulary/routes.ts` — HTTP endpoints
- `apps/web/src/routes/vocabulary.tsx` — vocabulary management page

### Modified files
- `packages/db/src/schema/index.ts` — export vocabulary schema
- `packages/db/src/repository/index.ts` — export vocabulary repository
- `packages/api/src/index.ts` — register vocabulary routes
- `packages/api/src/requirements/service.ts` — integrate vocabulary parsing into create/update
- `apps/web/src/routes/__root.tsx` — add Vocabulary nav link
- `apps/web/src/features/nav/mobile-nav.tsx` — add Vocabulary nav link
- `apps/web/src/routes/requirements_.new.tsx` — enhanced step builder with real-time parsing
- `apps/web/src/routes/requirements_.$requirementId.tsx` — show vocabulary warnings

---

## Chunk 1: Schema + Parser

### Task 1: Vocabulary schema

**Files:**
- Create: `packages/db/src/schema/vocabulary.ts`
- Create: `packages/db/src/__tests__/vocabulary.test.ts`
- Modify: `packages/db/src/schema/index.ts`

- [ ] **Step 1: Write schema test**

```typescript
// packages/db/src/__tests__/vocabulary.test.ts
import { getTableColumns } from "drizzle-orm";
import { describe, expect, it } from "vitest";
import { vocabularyEntry } from "../schema/vocabulary";

describe("vocabularyEntry table", () => {
    it("has expected columns", () => {
        const columns = getTableColumns(vocabularyEntry);
        expect(columns).toHaveProperty("id");
        expect(columns).toHaveProperty("word");
        expect(columns).toHaveProperty("category");
        expect(columns).toHaveProperty("expects");
        expect(columns).toHaveProperty("codebaseId");
        expect(columns).toHaveProperty("userId");
        expect(columns).toHaveProperty("createdAt");
        expect(columns).toHaveProperty("updatedAt");
    });
});
```

- [ ] **Step 2: Write schema**

```typescript
// packages/db/src/schema/vocabulary.ts
import { relations, sql } from "drizzle-orm";
import { index, jsonb, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";
import { user } from "./auth";
import { codebase } from "./codebase";

export const vocabularyEntry = pgTable(
    "vocabulary_entry",
    {
        id: text("id").primaryKey(),
        word: text("word").notNull(),
        category: text("category").notNull(),
        expects: jsonb("expects").$type<string[]>(),
        codebaseId: text("codebase_id")
            .notNull()
            .references(() => codebase.id, { onDelete: "cascade" }),
        userId: text("user_id").references(() => user.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        uniqueIndex("vocabulary_codebase_word_cat_idx")
            .on(table.codebaseId, table.category, sql`lower(${table.word})`),
        index("vocabulary_codebaseId_idx").on(table.codebaseId)
    ]
);

export const vocabularyEntryRelations = relations(vocabularyEntry, ({ one }) => ({
    codebase: one(codebase, { fields: [vocabularyEntry.codebaseId], references: [codebase.id] }),
    user: one(user, { fields: [vocabularyEntry.userId], references: [user.id] })
}));
```

Note: The unique index uses `lower(word)` for case-insensitive uniqueness. If Drizzle doesn't support `sql` in `uniqueIndex.on()`, use a raw SQL migration to add the index.

- [ ] **Step 3: Export, test, push**

Add `export * from "./vocabulary";` to `packages/db/src/schema/index.ts`. Run tests. Push schema.

- [ ] **Step 4: Commit**

```bash
git add packages/db/src/schema/vocabulary.ts packages/db/src/__tests__/vocabulary.test.ts packages/db/src/schema/index.ts
git commit -m "feat(db): add vocabulary_entry schema with case-insensitive unique index"
```

---

### Task 2: Vocabulary parser

**Files:**
- Create: `packages/api/src/vocabulary/parser.ts`
- Create: `packages/api/src/vocabulary/parser.test.ts`

- [ ] **Step 1: Write parser tests**

```typescript
// packages/api/src/vocabulary/parser.test.ts
import { describe, expect, it } from "vitest";
import { parseStepText } from "./parser";

const vocab = [
    { word: "user", category: "actor", expects: null },
    { word: "admin", category: "actor", expects: null },
    { word: "logged in", category: "state", expects: null },
    { word: "creates", category: "action", expects: ["target"] },
    { word: "sees", category: "outcome", expects: ["target", "state"] },
    { word: "chunk", category: "target", expects: null },
    { word: "dashboard", category: "target", expects: null },
    { word: "a", category: "modifier", expects: null },
    { word: "the", category: "modifier", expects: null },
    { word: "is", category: "modifier", expects: null },
    { word: "they", category: "modifier", expects: null },
];

describe("parseStepText", () => {
    it("parses a valid step with no warnings", () => {
        const result = parseStepText("a user creates the chunk", vocab);
        expect(result.warnings).toHaveLength(0);
        expect(result.tokens.map(t => t.category)).toEqual(["modifier", "actor", "action", "modifier", "target"]);
    });

    it("matches multi-word entries greedily", () => {
        const result = parseStepText("user is logged in", vocab);
        const categories = result.tokens.map(t => t.category);
        expect(categories).toContain("state"); // "logged in" matched as one token
    });

    it("warns on unknown words", () => {
        const result = parseStepText("a user frobnicates the chunk", vocab);
        const unknowns = result.warnings.filter(w => w.type === "unknown_word");
        expect(unknowns).toHaveLength(1);
        expect(unknowns[0].word).toBe("frobnicates");
    });

    it("warns on unexpected category", () => {
        const result = parseStepText("they creates the logged in", vocab);
        const unexpected = result.warnings.filter(w => w.type === "unexpected_category");
        expect(unexpected).toHaveLength(1);
        expect(unexpected[0].word).toBe("logged in");
    });

    it("warns on dangling expects at end of step", () => {
        const result = parseStepText("user creates", vocab);
        const dangling = result.warnings.filter(w => w.type === "expects_not_satisfied");
        expect(dangling).toHaveLength(1);
    });

    it("handles quoted literals", () => {
        const result = parseStepText('user sees "hello world"', vocab);
        expect(result.warnings.filter(w => w.type === "unknown_word")).toHaveLength(0);
    });

    it("is case insensitive", () => {
        const result = parseStepText("A User Creates The Chunk", vocab);
        expect(result.warnings.filter(w => w.type === "unknown_word")).toHaveLength(0);
    });

    it("returns empty result for empty text", () => {
        const result = parseStepText("", vocab);
        expect(result.tokens).toHaveLength(0);
        expect(result.warnings).toHaveLength(0);
    });
});
```

- [ ] **Step 2: Implement parser**

```typescript
// packages/api/src/vocabulary/parser.ts

interface VocabEntry {
    word: string;
    category: string;
    expects: string[] | null;
}

export interface ParsedToken {
    text: string;
    category: string | null;
    position: { start: number; end: number };
}

export interface VocabularyWarning {
    position: { start: number; end: number };
    type: "unknown_word" | "unexpected_category" | "expects_not_satisfied";
    word: string;
    message: string;
}

export interface ParseResult {
    tokens: ParsedToken[];
    warnings: VocabularyWarning[];
}

export function parseStepText(text: string, vocabulary: VocabEntry[]): ParseResult {
    if (!text.trim()) return { tokens: [], warnings: [] };

    const tokens: ParsedToken[] = [];
    const warnings: VocabularyWarning[] = [];

    // Step 1: Extract quoted literals and numbers, track their positions
    const literals: { start: number; end: number; text: string }[] = [];
    const processed = text.replace(
        /("[^"]*"|'[^']*'|\b\d+(?:\.\d+)?\b)/g,
        (match, _group, offset) => {
            literals.push({ start: offset, end: offset + match.length, text: match });
            return " ".repeat(match.length); // preserve positions
        }
    );

    // Step 2: Lowercase for matching
    const lower = processed.toLowerCase();

    // Step 3: Sort vocabulary by word length descending (greedy)
    const sorted = [...vocabulary]
        .filter(v => v.category !== "literal")
        .sort((a, b) => b.word.length - a.word.length);

    // Step 4: Scan left-to-right
    let pos = 0;
    while (pos < lower.length) {
        // Skip whitespace
        if (lower[pos] === " ") { pos++; continue; }

        // Check if we're at a literal placeholder position
        const literal = literals.find(l => l.start === pos);
        if (literal) {
            tokens.push({
                text: literal.text,
                category: "literal",
                position: { start: literal.start, end: literal.end }
            });
            pos = literal.end;
            continue;
        }

        // Try longest vocab match
        let matched = false;
        for (const entry of sorted) {
            const word = entry.word.toLowerCase();
            if (lower.startsWith(word, pos)) {
                // Check word boundary
                const endPos = pos + word.length;
                if (endPos < lower.length && lower[endPos] !== " ") continue;

                tokens.push({
                    text: text.slice(pos, endPos),
                    category: entry.category,
                    position: { start: pos, end: endPos }
                });
                pos = endPos;
                matched = true;
                break;
            }
        }

        if (!matched) {
            // Consume one word as unknown
            const wordEnd = lower.indexOf(" ", pos);
            const end = wordEnd === -1 ? lower.length : wordEnd;
            const word = text.slice(pos, end);
            tokens.push({
                text: word,
                category: null,
                position: { start: pos, end }
            });
            warnings.push({
                position: { start: pos, end },
                type: "unknown_word",
                word,
                message: `Unknown word: "${word}"`
            });
            pos = end;
        }
    }

    // Step 5: Validate slot expectations
    let lastExpects: { expects: string[]; word: string; position: { start: number; end: number } } | null = null;

    for (const token of tokens) {
        if (token.category === "modifier") continue;

        if (lastExpects && token.category) {
            if (token.category === "literal" || lastExpects.expects.includes(token.category)) {
                lastExpects = null; // satisfied
            } else {
                warnings.push({
                    position: token.position,
                    type: "unexpected_category",
                    word: token.text,
                    message: `"${lastExpects.word}" expects [${lastExpects.expects.join(", ")}], got ${token.category}`
                });
                lastExpects = null;
            }
        } else {
            lastExpects = null;
        }

        // Set new expects from this token
        const entry = sorted.find(e => e.word.toLowerCase() === (token.text?.toLowerCase() ?? ""));
        if (entry?.expects && entry.expects.length > 0) {
            lastExpects = { expects: entry.expects, word: token.text, position: token.position };
        }
    }

    // End-of-sequence check
    if (lastExpects) {
        warnings.push({
            position: lastExpects.position,
            type: "expects_not_satisfied",
            word: lastExpects.word,
            message: `"${lastExpects.word}" expects [${lastExpects.expects.join(", ")}] but step ended`
        });
    }

    return { tokens, warnings };
}
```

- [ ] **Step 3: Run tests**

Run: `cd packages/api && pnpm vitest run src/vocabulary/parser.test.ts`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/vocabulary/parser.ts packages/api/src/vocabulary/parser.test.ts
git commit -m "feat(api): add vocabulary parser with greedy tokenizer and slot validation"
```

---

## Chunk 2: Repository + Service + Routes

### Task 3: Vocabulary repository

**Files:**
- Create: `packages/db/src/repository/vocabulary.ts`
- Modify: `packages/db/src/repository/index.ts`

- [ ] **Step 1: Write repository**

Functions (all `Effect<T, DatabaseError>`):
- `listVocabulary(codebaseId)` — returns all entries for a codebase, ordered by category then word
- `createVocabularyEntry(params: { id, word, category, expects?, codebaseId, userId })` — lowercase word on insert
- `createVocabularyEntries(entries[])` — bulk insert with `onConflictDoNothing`
- `updateVocabularyEntry(id, params: { word?, category?, expects? })` — lowercase word if provided
- `deleteVocabularyEntry(id)`
- `getVocabularyEntry(id)`

Also a helper: `seedModifiers(codebaseId, userId)` — inserts the standard modifier set (`a`, `an`, `the`, `is`, `are`, `was`, `were`, `with`, `on`, `to`, `their`, `not`, `has`, `have`, `they`, `it`) with `onConflictDoNothing`.

- [ ] **Step 2: Export, run tests**

Add `export * from "./vocabulary";` to `packages/db/src/repository/index.ts`. Run tests.

- [ ] **Step 3: Commit**

```bash
git add packages/db/src/repository/vocabulary.ts packages/db/src/repository/index.ts
git commit -m "feat(db): add vocabulary repository with CRUD, bulk insert, and modifier seeding"
```

---

### Task 4: AI suggestion module

**Files:**
- Create: `packages/api/src/vocabulary/suggest.ts`

- [ ] **Step 1: Write suggestion module**

Reads chunks for a codebase, sends to Ollama for vocabulary extraction.

```typescript
// packages/api/src/vocabulary/suggest.ts
import { Effect } from "effect";

interface SuggestedEntry {
    word: string;
    category: string;
    expects: string[] | null;
}

export function suggestVocabulary(
    chunks: { title: string; content: string }[],
    ollamaUrl: string
): Effect.Effect<SuggestedEntry[], never> {
    return Effect.tryPromise({
        try: async () => {
            // Truncate content to fit context
            const context = chunks
                .map(c => `${c.title}\n${c.content.slice(0, 500)}`)
                .join("\n---\n")
                .slice(0, 8000);

            const prompt = `Extract domain vocabulary from these knowledge base entries.
For each word/phrase, classify as one of: actor, action, target, outcome, state.
For actions and outcomes, suggest what category they expect to follow.
Focus on domain-specific terms. Do not include common English words like "the", "a", "is".
Return ONLY a JSON array: [{ "word": "...", "category": "...", "expects": ["..."] | null }]

Entries:
${context}`;

            const res = await fetch(`${ollamaUrl}/api/generate`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ model: "llama3.2", prompt, stream: false })
            });

            if (!res.ok) return [];
            const data = await res.json();
            const text = data.response ?? "";

            // Extract JSON array from response
            const match = text.match(/\[[\s\S]*\]/);
            if (!match) return [];

            const parsed = JSON.parse(match[0]);
            if (!Array.isArray(parsed)) return [];

            // Validate and clean entries
            const validCategories = new Set(["actor", "action", "target", "outcome", "state"]);
            return parsed.filter(
                (e: any) =>
                    typeof e.word === "string" &&
                    validCategories.has(e.category) &&
                    e.word.trim().length > 0
            ).map((e: any) => ({
                word: e.word.toLowerCase().trim(),
                category: e.category,
                expects: Array.isArray(e.expects) ? e.expects : null
            }));
        },
        catch: () => [] as SuggestedEntry[]
    }).pipe(Effect.catchAll(() => Effect.succeed([] as SuggestedEntry[])));
}
```

- [ ] **Step 2: Commit**

```bash
git add packages/api/src/vocabulary/suggest.ts
git commit -m "feat(api): add AI vocabulary suggestion via Ollama"
```

---

### Task 5: Vocabulary service and routes

**Files:**
- Create: `packages/api/src/vocabulary/service.ts`
- Create: `packages/api/src/vocabulary/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Write service**

Functions:
- `listVocabulary(userId, codebaseId)` — verify user owns codebase, delegate to repo
- `createEntry(userId, body: { word, category, expects?, codebaseId })` — verify codebase ownership, lowercase word, create. If this is the first entry for the codebase, auto-seed modifiers first.
- `createEntries(userId, body: { entries[], codebaseId })` — bulk version for accepting AI suggestions
- `updateEntry(id, userId, body)` — verify entry exists, verify codebase ownership
- `deleteEntry(id, userId)` — verify entry exists, verify codebase ownership
- `parseStep(userId, body: { text, codebaseId })` — verify codebase ownership, fetch vocabulary, run parser, return ParseResult
- `suggestFromChunks(userId, codebaseId)` — verify codebase ownership, fetch chunks, call AI suggestion, return suggestions (not auto-insert)

- [ ] **Step 2: Write routes**

Route declaration order:
1. `GET /vocabulary` (`?codebaseId=`)
2. `POST /vocabulary/suggest` (body: `{ codebaseId }`)
3. `POST /vocabulary/bulk` (body: `{ entries[], codebaseId }`)
4. `POST /vocabulary/parse` (body: `{ text, codebaseId }`)
5. `POST /vocabulary` (single entry)
6. `PATCH /vocabulary/:id`
7. `DELETE /vocabulary/:id`

All routes use `requireSession(ctx).pipe(...)`.

Validation:
- category: `t.Union([t.Literal("actor"), t.Literal("action"), t.Literal("target"), t.Literal("outcome"), t.Literal("state"), t.Literal("modifier")])`
- expects: `t.Optional(t.Array(t.String(), { maxItems: 6 }))`
- bulk entries: `t.Array(..., { maxItems: 200 })`

- [ ] **Step 3: Register routes**

Import and `.use(vocabularyRoutes)` in `packages/api/src/index.ts`.

- [ ] **Step 4: Run tests**

Run: `cd packages/api && pnpm vitest run`

- [ ] **Step 5: Commit**

```bash
git add packages/api/src/vocabulary/service.ts packages/api/src/vocabulary/routes.ts packages/api/src/index.ts
git commit -m "feat(api): add vocabulary service and routes with parsing and AI suggestion"
```

---

### Task 6: Integrate vocabulary parsing into requirements

**Files:**
- Modify: `packages/api/src/requirements/service.ts`

- [ ] **Step 1: Add vocabulary parsing to createRequirement and updateRequirement**

After step sequence validation, for each step, call the vocabulary parser. Collect all vocabulary warnings. Return alongside existing cross-ref warnings.

```typescript
import { listVocabulary } from "@fubbik/db/repository";
import { parseStepText } from "../vocabulary/parser";

// In createRequirement, after cross-reference:
// Fetch vocabulary for the codebase (if codebaseId is set)
// Parse each step, collect warnings
// Return { requirement, warnings, vocabularyWarnings }

// In updateRequirement, same logic when steps are present
```

The vocabulary fetch must be wrapped so failure doesn't block the save — catch errors and return empty vocabulary warnings.

- [ ] **Step 2: Run tests**

Run: `cd packages/api && pnpm vitest run`

- [ ] **Step 3: Commit**

```bash
git add packages/api/src/requirements/service.ts
git commit -m "feat(api): integrate vocabulary parsing into requirements create/update"
```

---

## Chunk 3: Web UI

### Task 7: Vocabulary management page

**Files:**
- Create: `apps/web/src/routes/vocabulary.tsx`
- Modify: `apps/web/src/routes/__root.tsx`
- Modify: `apps/web/src/features/nav/mobile-nav.tsx`

- [ ] **Step 1: Read existing management pages**

Read `apps/web/src/routes/templates.tsx` or `apps/web/src/routes/codebases.tsx` for patterns.

- [ ] **Step 2: Create vocabulary page**

`/vocabulary` route:
- Only useful when a codebase is active (show message if none selected)
- Fetch entries: `api.api.vocabulary.get({ query: { codebaseId } })`
- Table grouped by category (collapsible sections), each row: word, category badge (color-coded), expects badges
- Add entry form: word input, category dropdown, expects multi-select (checkboxes)
- Edit inline, delete with confirmation
- "Suggest from chunks" button → calls `api.api.vocabulary.suggest.post({ codebaseId })` → shows review list with checkboxes → "Add selected" bulk inserts via `api.api.vocabulary.bulk.post()`
- Stats: total entries per category

- [ ] **Step 3: Add nav link**

"Vocabulary" link in nav. Consider showing only when a codebase is active, or always with a message.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/vocabulary.tsx apps/web/src/routes/__root.tsx apps/web/src/features/nav/mobile-nav.tsx apps/web/src/routeTree.gen.ts
git commit -m "feat(web): add vocabulary management page with AI suggestion and bulk add"
```

---

### Task 8: Enhanced requirement step builder

**Files:**
- Modify: `apps/web/src/routes/requirements_.new.tsx`

- [ ] **Step 1: Add real-time vocabulary parsing to step builder**

In the requirement create form's step builder:
- After each step text input, debounce (300ms) and call `POST /api/vocabulary/parse` with the step text and codebaseId
- Display token highlights:
  - Green background: recognized vocabulary word
  - Yellow background: unknown word, with small "Add?" button
  - Red underline: unexpected category violation
- "Add?" button opens an inline mini-form: category dropdown + optional expects. On submit, `POST /api/vocabulary`, then re-parse.

Implementation approach: below each step text input, render a row of colored token badges showing the parse result. This is simpler than trying to highlight within the input itself.

- [ ] **Step 2: Add auto-complete (optional enhancement)**

If time permits: show a filtered dropdown of vocabulary entries as the user types 2+ characters. Filter by the previous token's `expects` if available. This can be a follow-up task if it adds too much complexity.

- [ ] **Step 3: Show vocabulary warnings on requirement detail page**

In `apps/web/src/routes/requirements_.$requirementId.tsx`, if the response includes `vocabularyWarnings`, show them alongside cross-ref warnings in the yellow alert section.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/requirements_.new.tsx apps/web/src/routes/requirements_.$requirementId.tsx
git commit -m "feat(web): add real-time vocabulary parsing to requirement step builder"
```

---

### Task 9: Final verification

- [ ] **Step 1: Run type check**

Run: `pnpm run check-types`

- [ ] **Step 2: Run tests**

Run: `pnpm test`

- [ ] **Step 3: Push schema**

Run: `pnpm db:push`

- [ ] **Step 4: Fix any issues and commit**

```bash
git add -A && git commit -m "fix: resolve issues from controlled vocabulary implementation"
```
