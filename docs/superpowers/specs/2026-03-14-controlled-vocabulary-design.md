# Controlled Vocabulary for Requirements — Design Spec

## Overview

A per-codebase controlled vocabulary that defines the valid words and grammar for requirement steps. Each vocabulary entry has a category (actor, action, target, outcome, state, modifier) and optional typed slot expectations (e.g., "creates" expects a "target" to follow). The parser validates step text against the vocabulary, producing warnings for unknown words and unexpected category sequences. AI-powered seeding suggests vocabulary entries from existing chunks.

This extends the requirements system (spec: `2026-03-14-requirements-system-design.md`) with parseable, machine-verifiable steps.

## Data Model

### New `vocabulary_entry` table

| Column | Type | Notes |
|--------|------|-------|
| `id` | text (PK) | UUID as text |
| `word` | text | The word or phrase, e.g. "creates", "logged in", "chunk" |
| `category` | text | `actor`, `action`, `target`, `outcome`, `state`, `modifier`, `literal` |
| `expects` | jsonb (nullable) | Array of categories this word expects to follow, e.g. `["target"]`. Null = no expectation. |
| `codebaseId` | text (FK → codebase) | ON DELETE CASCADE |
| `userId` | text (FK → user, nullable) | ON DELETE SET NULL (entry persists if creator is deleted) |
| `createdAt` | timestamp | |
| `updatedAt` | timestamp | Auto-updated via Drizzle `$onUpdate(() => new Date())` |

**Constraints:**
- Unique on `(codebaseId, lower(word), category)` — case-insensitive uniqueness. Words are stored lowercase.
- Index on `codebaseId`

**Authorization:** All repository operations (list, create, update, delete) scope to the authenticated user's codebase ownership. The service verifies the user owns the codebase before any vocabulary operation.

**Case sensitivity:** All matching is case-insensitive. Words are lowercased on insert and during tokenization.

### Categories

| Category | Purpose | Examples |
|----------|---------|---------|
| `actor` | Who performs the action | user, admin, system, service |
| `action` | What they do | clicks, creates, deletes, visits, searches |
| `target` | What they act on | chunk, codebase, tag, dashboard, connection |
| `outcome` | What they observe | sees, receives, is redirected to, is notified |
| `state` | A condition | logged in, on the dashboard, has 3 chunks |
| `modifier` | Connecting words (transparent to grammar) | a, an, the, with, on, their, not, is |
| `literal` | Quoted values and numbers | Matched by pattern, no dictionary entry needed |

### Slot expectations

Each entry can define `expects: string[]` — the categories that should follow this word. Examples:

```json
{ "word": "creates", "category": "action", "expects": ["target"] }
{ "word": "sees", "category": "outcome", "expects": ["target", "state"] }
{ "word": "visits", "category": "action", "expects": ["target", "literal"] }
{ "word": "user", "category": "actor", "expects": null }
{ "word": "the", "category": "modifier", "expects": null }
```

Modifiers are transparent — they don't consume or satisfy `expects`. When checking expectations, the parser skips modifiers and checks the next non-modifier token.

Literals (quoted strings like `"hello"`, `'world'`, and numbers) satisfy any `expects` category — they're universal wildcards.

## Parser

The parser validates a requirement step's text against the vocabulary for a given codebase.

### Tokenization

1. Extract quoted strings and numbers, record their original positions, replace with `<literal_N>` indexed placeholders (preserving character offsets for position mapping)
2. Lowercase the remaining text
3. Sort vocabulary entries by word length descending (longest first)
4. Scan left-to-right. At each position, try to match the longest vocabulary entry. On match, consume those characters and record the token with its **original-text position** (mapped back from placeholder offsets). On no match, consume one whitespace-delimited word and flag as `unknown_word`.
5. After tokenization, replace `<literal_N>` tokens with the original quoted values, preserving their recorded positions

This greedy longest-match approach ensures "logged in user" matches `["logged in", "user"]` not `["logged", "in", "user"]`, because "logged in" (9 chars) is tried before "logged" (6 chars).

### Validation pass

For each matched token (left-to-right):
- If the previous non-modifier token had `expects`, check that the current token's category is in the expected list
- If not, produce an `unexpected_category` warning
- Modifiers are skipped in this check — they never consume or satisfy expectations
- **End-of-sequence check:** After processing all tokens, if the last non-modifier token had `expects` that was never satisfied, produce an `expects_not_satisfied` warning (e.g., "creates" expects [target] but step ended)

### Example

Step: `"Given a user creates the chunk"`

```
"a"       → modifier (transparent)
"user"    → actor (no expects) ✓
"creates" → action (expects: ["target"]) ✓
"the"     → modifier (transparent)
"chunk"   → target ✓ (satisfies "creates" expects target)
```

Step: `"When they creates the logged in"` (invalid)

```
"they"      → modifier (transparent)
"creates"   → action (expects: ["target"])
"the"       → modifier (transparent)
"logged in" → state ⚠ warning: "creates" expects [target], got state
```

### Response shape

```typescript
interface ParsedToken {
    text: string;
    category: string | null;  // null if unrecognized
    position: { start: number; end: number };
}

interface VocabularyWarning {
    position: { start: number; end: number };
    type: "unknown_word" | "unexpected_category" | "expects_not_satisfied";
    word: string;
    message: string;
}

interface ParseResult {
    tokens: ParsedToken[];
    warnings: VocabularyWarning[];
}
```

### Integration with requirements

When a requirement is created or updated (and `steps` is present in the body):
1. Step sequence validation runs first (Given/When/Then order — existing validator)
2. Vocabulary parsing runs on each step's text
3. Both sets of warnings are returned in the response

Vocabulary warnings do NOT block saving — they are informational. If the vocabulary fetch itself fails (DB error), the save proceeds with no vocabulary warnings rather than failing the entire operation.

**Step text vs keyword:** The `POST /vocabulary/parse` endpoint receives the step's `text` field only (not the keyword). The keyword (`given`, `when`, `then`) is structural metadata, not part of the vocabulary. The parser does not need to recognize keywords as vocabulary entries.

**Modifier pre-seeding:** When the first vocabulary entry is added to a codebase, a standard set of modifiers is auto-inserted: `a`, `an`, `the`, `is`, `are`, `was`, `were`, `with`, `on`, `to`, `their`, `not`, `has`, `have`, `they`, `it`. This prevents every codebase from manually adding common connecting words.

**Real-time parsing debounce:** The client-side parse call should be debounced at 300ms to avoid excessive API calls while typing.

## AI-Suggested Seeding

When a codebase's vocabulary is empty or on demand, fubbik can suggest entries by analyzing existing chunks.

### Flow

1. `POST /api/vocabulary/suggest?codebaseId=<id>`
2. Fetch all chunks for the codebase (titles + content, truncated to fit context)
3. Send to Ollama (llama3.2) with a structured prompt:

```
Extract domain vocabulary from these knowledge base entries.
For each word/phrase, classify as one of: actor, action, target, outcome, state.
For actions and outcomes, suggest what category they expect to follow.

Return as JSON array:
[{ "word": "...", "category": "...", "expects": ["..."] | null }]

Focus on domain-specific terms. Do not include common English words like "the", "a", "is".
```

4. Parse LLM response as JSON
5. Return suggestions to the client — **does NOT auto-insert**
6. User reviews, selects, and confirms which entries to add

### Fallback

If Ollama is not running, the endpoint returns `{ suggestions: [], message: "Ollama is not available" }`. The vocabulary works without AI — users add entries manually.

## API

### New endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/vocabulary?codebaseId=` | List all entries for a codebase |
| `POST` | `/vocabulary` | Add entry |
| `POST` | `/vocabulary/bulk` | Add multiple entries at once (for accepting AI suggestions) |
| `PATCH` | `/vocabulary/:id` | Update entry |
| `DELETE` | `/vocabulary/:id` | Remove entry |
| `POST` | `/vocabulary/suggest` | AI-suggested entries (body: `{ codebaseId }`, requires Ollama) |
| `POST` | `/vocabulary/parse` | Parse step text (body: `{ text, codebaseId }`, requires auth) |

### Modified endpoints

- `POST /requirements` — after step validation, runs vocabulary parsing on each step. Response includes `vocabularyWarnings` alongside existing `warnings`.
- `PATCH /requirements` — same, when `steps` is present.

### Route ordering

`/vocabulary/suggest`, `/vocabulary/bulk`, and `/vocabulary/parse` must be declared before `/vocabulary/:id`.

### Request/response examples

**POST /vocabulary:**
```json
{
    "word": "creates",
    "category": "action",
    "expects": ["target"],
    "codebaseId": "abc-123"
}
```

**POST /vocabulary/parse:**
```json
{
    "text": "a user creates the chunk",
    "codebaseId": "abc-123"
}
```
→ Returns `ParseResult` with tokens and warnings.

**POST /vocabulary/suggest:**
```json
{ "codebaseId": "abc-123" }
```
→ Returns `{ suggestions: [{ word, category, expects }...] }`

### Backend pattern

- `packages/db/src/schema/vocabulary.ts` — vocabulary_entry table
- `packages/db/src/repository/vocabulary.ts` — CRUD + list by codebase
- `packages/api/src/vocabulary/parser.ts` — tokenizer + validator
- `packages/api/src/vocabulary/parser.test.ts` — parser tests
- `packages/api/src/vocabulary/suggest.ts` — AI suggestion via Ollama
- `packages/api/src/vocabulary/service.ts` — composes CRUD + parsing + suggestion
- `packages/api/src/vocabulary/routes.ts` — HTTP endpoints

## Web UI

### Vocabulary management page (`/vocabulary`)

- Only visible/accessible when a codebase is selected
- Table of entries grouped by category (collapsible sections)
- Each row: word, category badge (color-coded), expects badges
- Add entry form: word input, category dropdown, expects multi-select (checkboxes for each category)
- Edit inline, delete with confirmation
- "Suggest from chunks" button → calls AI suggestion → shows review list with checkboxes → "Add selected" button
- Stats: total entries per category

### Enhanced requirement step builder

The existing step builder (in requirement create/edit pages) gets vocabulary-aware features:

**Real-time parsing:** As the user types step text, run vocabulary parsing (debounced, client-side call to `POST /vocabulary/parse`):
- Green highlight: recognized word
- Yellow highlight: unknown word, with inline "Add to vocabulary?" action button
- Red underline: unexpected category violation

**Auto-complete:** After typing 2+ characters, show a dropdown of matching vocabulary entries. If the previous token has `expects`, filter suggestions to only those categories.

**"Add to vocabulary" quick action:** Clicking the yellow "Add?" button on an unknown word opens a small inline form (category dropdown + optional expects). Submits to `POST /vocabulary`, then re-parses the step.

### Nav

Add "Vocabulary" link to navigation. Only shown when a codebase is active (since vocabulary is per-codebase).

## Future Considerations (Out of Scope)

- Vocabulary import/export between codebases
- Vocabulary versioning (track changes to the grammar over time)
- Synonym support ("clicks" and "presses" map to the same semantic action)
- Step templates composed from vocabulary ("Given <actor> is <state>" as a reusable pattern)
- Vocabulary-aware test generation (using category semantics to generate smarter test scaffolds)
