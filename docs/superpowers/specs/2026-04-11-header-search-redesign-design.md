# Header Search Bar Redesign

## Problem

The current header search is a ~160px `<input>` tucked between the "Docs" nav link and the "Manage" dropdown. It:

- Is barely visible and feels like an afterthought
- Only accepts plain text — users can't type advanced queries (`type:reference tag:api near:"auth flow"`) without navigating to the `/search` page first
- Has no autocomplete, no suggestions, no saved queries access
- Always navigates away on Enter, regardless of current context

The full query builder lives on `/search`, but it's a round trip away. Power users want advanced queries from anywhere.

## Goal

Replace the current header input with a larger `HeaderSearchBar` (~460px) that supports pill-based advanced queries, live autocomplete, saved/recent queries, and context-aware Enter behavior. Consolidate the primary nav from 7 links to 4 to make room.

---

## 1. Nav Consolidation

To free up horizontal space for the bigger search bar, reduce the primary nav from 7 links to 4.

**Keep in primary nav:**
- Dashboard
- Chunks
- Graph
- Requirements

**Move into the Manage dropdown:**
- Features
- Reviews
- Docs

The Manage dropdown gets a new top section labeled "Navigate" containing these three items, separated from the existing sections by a `DropdownMenuSeparator`.

This frees up ~240px in the nav row — enough for a ~460px search bar plus breathing room.

**Mobile:** No change — the `MobileNav` drawer already handles narrow screens.

---

## 2. Search Bar Anatomy

New component: `apps/web/src/features/nav/header-search-bar.tsx`

### Container

```tsx
<div className="flex-1 max-w-[460px] min-w-[280px] ..."> {/* grows to fill */}
    <div className="flex items-center gap-2 h-9 bg-muted/40 border border-border/50 rounded-md px-2 focus-within:ring-1 focus-within:ring-ring">
        <SearchIcon className="size-3.5 text-muted-foreground shrink-0" />
        <PillRow clauses={clauses} onRemove={removeClause} />
        <input
            ref={inputRef}
            type="text"
            value={rawInput}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            placeholder={clauses.length === 0 ? "Search…" : ""}
            className="flex-1 min-w-[80px] bg-transparent border-0 outline-none text-sm font-mono"
        />
        {!focused && (
            <kbd className="ml-auto text-[9px] text-muted-foreground font-mono border border-border/40 rounded px-1">
                /
            </kbd>
        )}
    </div>
    <HeaderSearchDropdown
        open={focused && (clauses.length > 0 || rawInput.length > 0 || recentOrSavedAvailable)}
        rawInput={rawInput}
        clauses={clauses}
        onSelect={handleSuggestionSelect}
        selectedIdx={selectedIdx}
    />
</div>
```

### Pills

Each clause renders as a `PillChip`:

```tsx
<span className={`inline-flex items-center gap-1 rounded border px-1.5 py-0.5 text-[10px] font-mono font-semibold ${FILTER_COLORS[clause.field] ?? SLATE_COLOR}`}>
    {clause.negate && <span>NOT</span>}
    <span>{clause.field}:{clause.value}</span>
    <button type="button" onMouseDown={e => { e.preventDefault(); onRemove(idx); }}>
        ×
    </button>
</span>
```

- Color map reuses `FILTER_COLORS` from `apps/web/src/features/search/query-types.ts`
- Backspace with empty input removes the last pill
- Click × removes that specific pill
- Pills are not editable inline (click × and retype)

### State

```typescript
const [clauses, setClauses] = useState<QueryClause[]>([]);
const [rawInput, setRawInput] = useState("");
const [focused, setFocused] = useState(false);
const [selectedIdx, setSelectedIdx] = useState(0);
```

### Sync with URL

- When mounted on `/search`, the bar reads `location.search.q` and parses it into clauses via `GET /api/search/parse?q=...`
- On other routes, pills reset on navigation (stateless header bar outside `/search`)

---

## 3. Autocomplete Dropdown

New component: `apps/web/src/features/nav/header-search-dropdown.tsx`

### Position

```tsx
<div className="absolute top-full left-0 right-0 mt-1 z-50 max-h-80 overflow-y-auto rounded-md border bg-card shadow-xl">
```

Anchored to the search bar via an absolute-positioned wrapper. The parent search bar container has `relative`.

### Content states

**State 1 — Empty input, no pills (focused):**

```
┌─ Saved queries ─────────────┐
│ ⭐ API docs needing review   │
│    type:reference review:draft │
│ ⭐ Stale architecture         │
│    type:document updated:90d │
├─ Recent ────────────────────┤
│ 🕐 tag:api connections:3+    │
│    2h ago                    │
│ 🕐 near:"Arch Overview"      │
│    yesterday                 │
└──────────────────────────────┘
```

- Saved queries: `GET /api/search/saved`, top 5
- Recent: `useRecentQueries` hook (sessionStorage-backed, ~15 entries)
- Click loads the query into the pill bar and focuses the input

**State 2 — Typing a field prefix** (e.g. `type`, `tag`, `near`):

```
┌─ Fields ────────────────────┐
│ type      Chunk type         │
│ tag       Filter by tag      │
│ text      Full-text search   │
│ near      Within N hops      │
│ path      Connection path    │
│ ...                          │
└──────────────────────────────┘
```

- Pulls from `FILTER_CATEGORIES` in `query-types.ts`
- Enter scaffolds the pill: sets `rawInput` to `<field>:` and keeps focus

**State 3 — Typing `field:value-prefix`** (e.g. `type:ref`):

| Field | Source |
|-------|--------|
| `type` | Hardcoded: note, document, reference, schema, checklist |
| `origin` | Hardcoded: human, ai |
| `review` | Hardcoded: draft, reviewed, approved |
| `tag` | `GET /api/search/autocomplete?field=tag&prefix=...` |
| `near`, `path`, `affected-by` | `GET /api/search/autocomplete?field=chunk&prefix=...` or `field=requirement&prefix=...` |

- Debounced 150ms
- Click or Enter creates the completed pill

**State 4 — Free text (no `:`)**:

```
┌─ Chunks ────────────────────┐
│ 📄 Auth Flow      reference  │
│ 📄 Session Mgmt   document   │
│ 📄 API Auth       reference  │
├─────────────────────────────┤
│ Search for "auth"            │
└──────────────────────────────┘
```

- Top 5 chunks by title match (debounced 150ms)
- Click a chunk → navigate to `/chunks/$id`
- Click "Search for '<text>'" → treat as Enter (text-search clause)

### Footer hint

```
↑↓ navigate · ⏎ select · ⇧⏎ full search
```

### Keyboard nav

- `↑` / `↓` — move selection
- `Enter` — select highlighted item
- `Tab` — accept highlighted suggestion into the input (without submitting)
- `Escape` — close dropdown, clear focus
- `Shift+Enter` — force navigate to `/search` even if a chunk is highlighted

---

## 4. Enter Behavior

### On `/search` page

- Update URL query param `?q=<serialized clauses>`
- The `/search` page parses the URL on change and updates its own query builder
- The header bar doesn't clear — stays in sync

### On any other page

- Navigate to `/search?q=<serialized clauses>` via `useNavigate`
- Header bar keeps its pills during navigation (persists via component state, not URL)
- When `/search` mounts, it reads the `q` param and loads the clauses into its internal builder

### Unclosed clause in input

- On Enter, try to parse the partial text via `GET /api/search/parse?q=...`
- If the parser returns valid clauses, apply them
- If not (e.g., trailing `type:` with nothing), show a subtle inline error below the bar: "Incomplete filter — pick a value" (auto-dismiss after 3s)

### Click a dropdown suggestion

| Suggestion type | Behavior |
|---|---|
| Field name | Scaffold `<field>:` in rawInput, keep focus |
| Value (type/origin/review) | Create pill, clear rawInput |
| Tag value | Create pill, clear rawInput |
| Chunk title (for `near:`/`path:`/`affected-by:`) | Create pill, clear rawInput |
| Chunk title (free text mode) | Navigate to `/chunks/$id` |
| "Search for '<text>'" | Same as Enter |
| Saved query | Load all clauses into pill bar |
| Recent query | Load all clauses into pill bar |

---

## 5. Files Changed

### New files

- `apps/web/src/features/nav/header-search-bar.tsx` — main component
- `apps/web/src/features/nav/header-search-dropdown.tsx` — autocomplete dropdown component
- `apps/web/src/hooks/use-recent-queries.ts` — sessionStorage hook for recent queries (~15 entries, same pattern as `use-reading-trail`)

### Modified

- `apps/web/src/routes/__root.tsx`:
  - Remove the current `<input>` and its local state (`navSearch`, `searchInputRef`, `/` keyboard listener)
  - Add `<HeaderSearchBar />` in its place
  - Remove `Features`, `Reviews`, `Docs` from the primary nav links
  - Add a new "Navigate" section at the top of the Manage dropdown containing the three removed items

### Unchanged (reused)

- `FILTER_COLORS`, `GRAPH_FIELDS`, `FILTER_CATEGORIES` from `apps/web/src/features/search/query-types.ts`
- `GET /api/search/parse` — reused for parsing the text input into clauses
- `GET /api/search/autocomplete?field=...&prefix=...` — reused for value autocomplete
- `GET /api/search/saved` — reused for saved queries
- `/search` page — unchanged, still the target of Enter in most cases. Its URL-param parsing already exists.

### Not modified

- `MobileNav` — mobile nav stays the same. Mobile search variant is out of scope.

---

## 6. Data Flow

```
User types "type:ref" in bar
    ↓
Component detects field:value-prefix pattern
    ↓
Fetches GET /api/search/autocomplete?field=type&prefix=ref
    (or uses hardcoded list for enum fields)
    ↓
Dropdown shows matches: "reference"
    ↓
User presses Enter or clicks
    ↓
Component creates PillClause { field: "type", operator: "is", value: "reference" }
    ↓
rawInput cleared, focus stays in input
    ↓
User continues typing or presses Enter again to submit
    ↓
Submit → navigate to /search?q=type:reference (or update URL if already there)
```

---

## 7. Out of Scope

- Pill editing (click to edit value inline)
- Drag-reordering pills
- Saving queries from the header bar (still only via `/search` page)
- Mobile search variant (separate future task)
- Query history beyond sessionStorage (persistent history across sessions)
- Natural-language search ("chunks about auth from last week")

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Users unfamiliar with syntax type free text and get confused | Free-text always falls back to text-search clause; dropdown shows matching chunks so it feels like a chunk picker first |
| Autocomplete fetches too frequently | Debounce 150ms on value queries; cache by prefix inside `useQuery` with `staleTime: 30_000` |
| Dropdown click dismisses focus before click handler fires | Use `onMouseDown` (fires before blur) instead of `onClick` on dropdown items |
| Long pill rows overflow the 460px bar | Pill container has `overflow-x-auto`, horizontal scroll for the whole row |
| Header bar and `/search` page get out of sync | When on `/search`, subscribe to URL `q` changes and update pill state. Two-way sync via the URL param |
| Parsing trailing `field:` returns an empty or broken clause | `/api/search/parse` rejects unclosed clauses; show inline error tooltip |

---

## Success Criteria

- Search bar visible at ~460px width on desktop screens ≥1024px
- Users can type `type:reference tag:api` and see pills form automatically
- Typing `type:` shows a dropdown with the 5 valid types
- Typing `tag:` shows live tag autocomplete
- Typing free text shows matching chunks in the dropdown
- Empty-state dropdown shows saved + recent queries
- Enter on `/search` updates in place; Enter elsewhere navigates
- Backspace with empty input removes last pill
- `/` keyboard shortcut focuses the bar from anywhere
- Escape closes dropdown and clears focus
- Nav primary links reduced from 7 to 4
