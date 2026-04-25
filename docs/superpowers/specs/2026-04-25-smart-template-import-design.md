# Smart Template-Based Import

**Date:** 2026-04-25
**Status:** Approved

## Problem

Every document imported into fubbik gets hardcoded to `type: "document"` with no structured field extraction. An ADR, a runbook, and a changelog all become the same kind of chunk. Templates exist for manual chunk creation but are never consulted during import. There's no way to tell fubbik "I know what my docs look like — use that knowledge to import them properly."

## Solution

Extend the existing template system with user-defined matching rules and field extraction mappings. When documents are imported (CLI or web), fubbik scores each doc against templates that have matching rules, suggests the best match, and extracts structured fields. The user confirms or overrides before import.

## Decisions

- **Persona:** Solo developer with a running server
- **Classification + extraction:** Templates both recognize doc types and extract structured fields
- **User-defined rules:** No AI/magic — users create explicit matching rules
- **Web UI-first:** Templates managed in the web UI, consulted by all import paths
- **Suggest and confirm:** Matches are shown in import preview; user confirms per file
- **Tag merging:** Template tags ∪ frontmatter tags ∪ path-derived tags (no override)

## Template Match Rules

Each template gains an optional `matchRules` field — a set of conditions scored against imported documents.

### Schema

```typescript
interface MatchRules {
  /** Minimum total score required for this template to match a doc */
  minScore: number;
  /** Heading patterns to look for in the document */
  headings: HeadingRule[];
  /** Frontmatter field expectations */
  frontmatter: FrontmatterRule[];
}

interface HeadingRule {
  /** One or more heading text patterns (alternatives — any match counts) */
  patterns: string[];
  /** How to match pattern against heading text. Default: "prefix" */
  match: "exact" | "prefix" | "contains";
  /** Heading level (2 = ##, 3 = ###). Omit to match any level. */
  level?: number;
  /** If true, doc must have this heading to match at all. Default: true */
  required: boolean;
}

interface FrontmatterRule {
  /** Frontmatter key to check */
  key: string;
  /** Match mode. Default: "exact" */
  match: "exact" | "oneOf" | "exists";
  /** Expected value (for "exact") or values (for "oneOf"). Ignored for "exists". */
  value?: string;
  values?: string[];
}
```

### Scoring

1. Check all required heading rules. If any required heading is missing → score = 0, no match.
2. Each required heading found: **+1 point**.
3. Each optional heading found: **+0.5 points**.
4. Each frontmatter rule matched: **+1 point**.
5. If total score < `minScore` → no match.
6. The template with the highest score wins. Ties broken by `priority` (higher wins), then by number of required rules (more specific wins).

### Examples

**ADR template matchRules:**
```json
{
  "minScore": 2,
  "headings": [
    { "patterns": ["Decision", "Choice", "Selected Option"], "match": "prefix", "level": 2, "required": true },
    { "patterns": ["Alternatives", "Options Considered"], "match": "prefix", "level": 2, "required": true },
    { "patterns": ["Consequences", "Impact"], "match": "prefix", "level": 2, "required": false },
    { "patterns": ["Context", "Background"], "match": "prefix", "level": 2, "required": false }
  ],
  "frontmatter": [
    { "key": "type", "match": "oneOf", "values": ["adr", "decision", "architecture-decision"] }
  ]
}
```

**Runbook template matchRules:**
```json
{
  "minScore": 2,
  "headings": [
    { "patterns": ["Prerequisites", "Requirements"], "match": "prefix", "level": 2, "required": true },
    { "patterns": ["Steps", "Procedure", "Instructions"], "match": "prefix", "level": 2, "required": true },
    { "patterns": ["Rollback", "Recovery"], "match": "prefix", "level": 2, "required": false },
    { "patterns": ["Troubleshooting"], "match": "prefix", "level": 2, "required": false }
  ],
  "frontmatter": [
    { "key": "type", "match": "oneOf", "values": ["runbook", "playbook"] }
  ]
}
```

## Field Extraction Mappings

Once a template matches, it can extract content from specific sections into structured chunk fields.

### Schema

```typescript
interface FieldMapping {
  /** Heading patterns to match (same alias approach as matchRules) */
  headings: string[];
  /** How to match pattern against heading text. Default: "prefix" */
  match: "exact" | "prefix" | "contains";
  /** Chunk field to populate with the matched section's content */
  target: "rationale" | "alternatives" | "consequences" | "summary" | "scope" | "content";
}
```

### Target field behavior

| Target | Chunk column | Extraction behavior |
|--------|-------------|-------------------|
| `rationale` | `rationale` (text) | Section content verbatim |
| `alternatives` | `alternatives` (text[]) | Split by bullet points (`- `) or sub-headings into array entries |
| `consequences` | `consequences` (text) | Section content verbatim |
| `summary` | `summary` (text) | Section content verbatim |
| `scope` | `scope` (JSONB) | Parse `key: value` lines into key-value object |
| `content` | `content` (text) | Section content replaces main body |

### Remaining content

Sections are partitioned into "extracted" (matched by a field mapping with a non-`content` target) and "unextracted" (everything else). The chunk's `content` field is built from all unextracted sections joined in their original order. This means extracted fields like `rationale` are *removed* from the body to avoid duplication, while everything else — including sections explicitly mapped to `content` — stays in the body.

### No mappings = classification only

A template with `matchRules` but no `fieldMappings` still classifies the doc (assigns type + tags) without extracting fields. The full body stays in `content`.

### Example: ADR field mappings

```json
[
  { "headings": ["Context", "Background"], "match": "prefix", "target": "content" },
  { "headings": ["Decision", "Choice"], "match": "prefix", "target": "rationale" },
  { "headings": ["Alternatives", "Options Considered"], "match": "prefix", "target": "alternatives" },
  { "headings": ["Consequences", "Impact"], "match": "prefix", "target": "consequences" }
]
```

## Tag Assignment

When a template matches, tags are merged from three sources (deduplicated):

```
finalTags = template.tags ∪ frontmatter.tags ∪ pathTags(filePath)
```

- **Template tags:** Stored on the template (e.g., `["architecture", "decision"]` for ADR)
- **Frontmatter tags:** From the doc's YAML frontmatter `tags:` field
- **Path tags:** Derived from folder segments (existing behavior: `docs/api/auth.md` → `["docs", "api", "auth"]`)

## Import Flow Integration

### Matching engine

A new service function in `packages/api/src/templates/`:

```typescript
function matchTemplates(
  parsedDoc: { headings: ParsedHeading[]; frontmatter: Record<string, unknown> },
  templates: TemplateWithRules[]
): TemplateMatch | null
```

- Takes a parsed document and all templates that have `matchRules`
- Scores each template
- Returns the best match above threshold, or null

### Preview endpoint

New endpoint: `POST /api/chunks/import-docs/preview`

**Request:** same as `import-docs` — `{ files: [...], codebaseId: string }`

**Response:** per file:
```typescript
{
  path: string;
  title: string;
  suggestedTemplate: {
    id: string;
    name: string;
    score: number;
    type: string;
    tags: string[];
    extractedFields: {
      rationale?: string;
      alternatives?: string[];
      consequences?: string;
      summary?: string;
      scope?: Record<string, string>;
    };
  } | null;
  parsed: {
    title: string;
    type: string;
    tags: string[];
    content: string;
  };
}
```

### Modified import endpoint

`POST /api/chunks/import-docs` gains an optional `templateOverrides` field:

```typescript
{
  files: [{ path: string; content: string }],
  codebaseId: string,
  templateOverrides?: Record<string, string | null>  // path → template ID (or null to skip)
}
```

When a file has a template override:
1. Extract fields using that template's `fieldMappings`
2. Apply that template's `type` and merge tags
3. Create chunk with extracted structured fields

When no override is provided, fall back to current behavior (type: "document", no extraction).

### Web import page

The existing `/import` page gains:

1. Upload files (unchanged)
2. Preview calls `POST /api/chunks/import-docs/preview`
3. Each row shows: filename, title, **template match dropdown** (suggested template pre-selected, with "None" option), collapsible extracted fields preview
4. User confirms or changes template per file
5. Import sends confirmed selections as `templateOverrides`

### CLI integration

`fubbik import --server` and `fubbik setup`:

- `--dry-run` shows template matches in preview output
- Interactive mode shows matches per file, asks for confirmation
- `--yes` auto-accepts all template suggestions

## Database Changes

### Modified table: `chunkTemplate`

Add columns:

| Column | Type | Default | Description |
|--------|------|---------|-------------|
| `matchRules` | JSONB, nullable | null | Heading + frontmatter matching rules |
| `fieldMappings` | JSONB, nullable | null | Heading → chunk field extraction mappings |
| `priority` | integer | 0 | Higher = preferred in match ties |
| `tags` | text[], nullable | null | Tags to apply when template matches |

No new tables. Templates without `matchRules` behave exactly as today.

### Migration

- Add columns via Drizzle schema update
- Seed built-in templates with matching rules:
  - **Decision Record** → matches `## Decision` + `## Alternatives`, extracts rationale/alternatives/consequences
  - **API Reference** → matches `## Endpoint` or `## Request`/`## Response`
  - **Meeting Notes** → matches `## Attendees` + `## Action Items`
  - **Checklist** → matches frontmatter `type: checklist`
  - **Schema** → matches frontmatter `type: schema`

## Template Management UI

### Enhanced template editor (on `/templates`)

**Existing fields** (unchanged): name, description, type, content scaffold.

**New "Import Matching" section** (collapsible):
- Toggle: "Use for import matching"
- **Tags**: tag input for template-level tags
- **Heading rules**: repeatable rows — pattern(s) input, match mode dropdown, level dropdown, required toggle
- **Frontmatter rules**: repeatable rows — key input, match mode dropdown, value(s) input
- **Min score**: number input (defaults to count of required rules)
- **Priority**: number input (default 0)

**New "Field Extraction" section** (collapsible):
- Repeatable rows — heading pattern(s) input, match mode dropdown, target field dropdown

**Test panel**:
- Textarea to paste sample markdown
- "Test Match" button
- Shows: matched/not matched, score, extracted fields with content preview
- Lets users iterate on rules without doing a real import

### Built-in template handling

Built-in templates display their matchRules and fieldMappings as read-only. Users can duplicate a built-in template to create an editable custom version.
