# Markdown Docs Import

Import a folder of markdown documents (with nested subdirectories) as chunks, via both a web UI and a CLI command.

## Decisions

- **One file = one chunk** — no heading-based splitting
- **Metadata extraction** — YAML frontmatter + folder structure as tags
- **Web UI** — both a quick dialog (chunks list page) and a dedicated `/import` page with preview
- **Codebase selection** — always required (user must select)
- **No auto-enrichment** — user enriches later manually
- **API-first** — single `POST /api/chunks/import-docs` endpoint handles all parsing server-side; CLI and web are thin clients

## API Endpoint

### `POST /api/chunks/import-docs`

**Request:**

```typescript
{
  files: Array<{
    path: string      // relative path preserving folder structure, e.g. "api/auth.md"
    content: string   // raw file content including frontmatter
  }>                  // max 500 files
  codebaseId: string  // required
}
```

**Response:**

```typescript
{
  created: number
  skipped: number     // files with no parseable content after frontmatter
  errors: Array<{ path: string, error: string }>
}
```

### Server-Side Parsing Per File

1. Parse YAML frontmatter (use `gray-matter` or lightweight equivalent)
2. Recognized frontmatter fields: `title` (string), `type` (string), `tags` (string[]), `scope` (object) — all optional
3. Title fallback chain: frontmatter `title` > first `# heading` in body > filename without `.md` extension
4. Folder-derived tags: each directory segment in the relative path becomes a tag (e.g., `guides/api/auth.md` produces tags `["guides", "api"]`)
5. Final tags = frontmatter tags + folder tags, deduplicated
6. Type defaults to `"document"` if not specified in frontmatter
7. Content = markdown body after frontmatter is stripped (and after leading `# heading` if it was used as title)
8. Skip files that have no content after processing (increment `skipped` count)
9. Create chunks via `chunkService.createChunk()` with concurrency of 10
10. Collect errors per file — do not fail the entire batch on individual errors

## CLI Command

### `fubbik import-docs <path> --codebase <name>`

- `<path>` — directory path, required
- `--codebase <name>` — codebase name, required
- Recursively reads all `.md` files from `<path>` and nested subdirectories
- Computes relative paths from `<path>` root
- Sends `{ files, codebaseId }` to `POST /api/chunks/import-docs`
- Resolves codebase name to ID via `GET /api/codebases/detect` or `GET /api/codebases`
- Outputs summary: `Created: N | Skipped: N | Errors: N`
- Lists individual errors if any
- Supports `--quiet` for machine-readable output (just the created count)

## Web UI

### Quick Dialog (Chunks List Page)

Add "Import Docs" option to the header dropdown menu (alongside existing actions).

**Dialog contents:**
- Folder picker via `<input type="file" webkitdirectory>` (also accepts multiple individual files)
- Codebase selector dropdown (required)
- File count preview after selection (e.g., "12 markdown files found")
- Submit button → calls API → toast with results summary

### Dedicated Page (`/import`)

Full-featured import experience:

- **Upload zone** — drag-and-drop area + folder picker button + individual file picker
- **Codebase selector** — required dropdown
- **Preview table** — columns: file path, detected title, detected tags, detected type
- **Row selection** — checkboxes to deselect individual files before importing
- **Summary bar** — "N files selected" with Import button
- **Results view** — after import: created/skipped/errors counts, error details expandable, link to chunks list filtered by codebase

### Navigation

- Add `/import` to the main nav or as a secondary link
- Add "Import Docs" to the chunks list page dropdown (opens dialog)

## File Structure (New/Modified)

### New Files

- `packages/api/src/chunks/parse-docs.ts` — frontmatter parsing, title extraction, folder-to-tags logic
- `packages/api/src/chunks/parse-docs.test.ts` — unit tests for parsing
- `apps/cli/src/commands/import-docs.ts` — CLI command
- `apps/web/src/routes/import.tsx` — dedicated import page
- `apps/web/src/features/import/import-dialog.tsx` — quick import dialog component

### Modified Files

- `packages/api/src/chunks/routes.ts` — add `import-docs` route
- `apps/cli/src/index.ts` — register `import-docs` command
- `apps/web/src/routes/chunks.index.tsx` — add "Import Docs" to dropdown menu
- `apps/web/src/components/nav.tsx` (or equivalent) — add `/import` link if appropriate

## Edge Cases

- **Non-.md files in folder** — silently ignored
- **Empty files** — counted as skipped
- **Duplicate titles** — allowed (chunks can have same title)
- **Large folders (>500 files)** — return 400 with message suggesting smaller batches
- **Binary files with .md extension** — frontmatter parser will fail, counted as error
- **Frontmatter with unknown fields** — ignored (only recognized fields extracted)
- **No frontmatter** — perfectly valid, title from heading/filename, type defaults to "document"
