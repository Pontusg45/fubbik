# Docs as Chunks — Design Spec

**Date:** 2026-04-02
**Status:** Draft

## Overview

Enable Fubbik to import markdown documentation, split it into chunks based on H2 headings, track the original document structure, and reconstruct documents as browsable pages in the web UI. This makes Fubbik a documentation browser backed by its knowledge graph.

## Goals

- Import a markdown file and split it into ordered chunks (one per H2 section)
- Track the original document (source path, content hash, chunk ordering)
- Re-import changed files with hash-based change detection and heading-based matching
- Render documents back as readable pages in a dedicated `/docs` route
- Provide CLI commands for import, sync, and rendering

## Non-Goals

- Bidirectional sync (editing a chunk does not write back to the source file)
- Splitting on heading levels other than H2 (can be added later)
- Inline editing in the docs viewer

---

## Database Schema

### New `document` Table

| Column | Type | Description |
|--------|------|-------------|
| `id` | uuid | PK |
| `title` | text | Document title (from H1 or filename) |
| `sourcePath` | text | File path relative to codebase root |
| `contentHash` | text | SHA-256 of the raw file content |
| `description` | text (nullable) | Optional description (from frontmatter or first paragraph) |
| `codebaseId` | uuid (nullable) | FK to `codebase` |
| `userId` | text | FK to `user` |
| `createdAt` | timestamp | |
| `updatedAt` | timestamp | |

**Unique constraint:** `(sourcePath, codebaseId, userId)` — prevents duplicate imports of the same file.

### Changes to `chunk` Table

Two new nullable columns:

| Column | Type | Description |
|--------|------|-------------|
| `documentId` | uuid (nullable) | FK to `document` |
| `documentOrder` | integer (nullable) | Position within the document (0-indexed) |

**Unique constraint:** `(documentId, documentOrder) WHERE documentId IS NOT NULL` — ensures ordering integrity.

Chunks without a `documentId` are unaffected. No breaking changes.

---

## Import & Splitting Logic

### Parsing Pipeline

1. Read the markdown file content
2. Extract frontmatter (existing logic) for tags, type, scope
3. Extract the H1 heading as the document title (fallback: filename without extension)
4. Split content on `## ` boundaries — each H2 section becomes a chunk
5. Content between the H1 and the first H2 becomes a "preamble" chunk at order 0 (if non-empty)
6. Each chunk receives:
   - `title` = the H2 heading text (preamble chunk gets `"{document title} — Introduction"`)
   - `content` = everything between this H2 and the next H2 (including H3+ subheadings within)
   - `documentOrder` = positional index (0-based)
   - `type` = `document` (existing chunk type)
   - Tags inherited from the document-level frontmatter
7. Create the `document` record with `sourcePath`, `contentHash`, `title`
8. Create all chunks linked via `documentId`

### Re-Import (Sync) Logic

1. Compute SHA-256 hash of the file — if unchanged from stored `contentHash`, skip
2. Re-parse the file into sections using the same pipeline
3. Match new sections to existing chunks by heading title (trimmed, case-insensitive comparison)
4. **Matched + changed content:** Update chunk content, preserve tags, connections, and enrichment
5. **New sections:** Create new chunks, insert at correct `documentOrder`
6. **Deleted sections:** Add a `stale` tag to flag for review (do not auto-delete, since users may have added connections or enrichment)
7. Reorder all `documentOrder` values to be sequential (0, 1, 2, ...)
8. Update document's `contentHash` and `updatedAt`

---

## API Endpoints

New route module at `packages/api/src/documents/routes.ts`.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/documents` | List documents (supports `codebaseId` filter) |
| `GET` | `/api/documents/:id` | Document detail with ordered chunks |
| `POST` | `/api/documents/import` | Import a markdown file as document + chunks |
| `POST` | `/api/documents/import-dir` | Import a directory of markdown files |
| `POST` | `/api/documents/:id/sync` | Re-import from source with hash-based diffing |
| `GET` | `/api/documents/:id/render` | Reconstruct document as a single markdown string |
| `DELETE` | `/api/documents/:id` | Delete document (chunks lose `documentId` but are not deleted) |

Existing `/api/chunks` endpoints are unchanged. Chunk responses include `documentId` and `documentOrder` when present.

### Architecture

Follows existing patterns:

- **Repository:** `packages/db/src/repository/document.ts` — pure data access returning `Effect<T, DatabaseError>`
- **Service:** `packages/api/src/documents/service.ts` — splitting, sync diffing, rendering logic
- **Routes:** `packages/api/src/documents/routes.ts` — HTTP layer calling service via Effect

---

## CLI Commands

New `docs` subcommand group in `apps/cli/src/commands/docs.ts`.

| Command | Description |
|---------|-------------|
| `fubbik docs import <path>` | Import a single markdown file as a document |
| `fubbik docs import-dir <dir>` | Import a directory of markdown files |
| `fubbik docs list` | List documents (supports `--codebase`) |
| `fubbik docs show <id>` | Show document with its chunk list |
| `fubbik docs sync [id]` | Re-import changed files. With ID: sync one. Without: sync all for current codebase |
| `fubbik docs render <id>` | Output reconstructed markdown to stdout |

The existing `fubbik import` command remains unchanged for non-document chunk imports.

---

## Web UI — `/docs` Route

### Sidebar

- Tree-structured index of documents, grouped by folder path segments (e.g., `docs/guides/` as a collapsible group)
- Each entry shows document title and chunk count
- Codebase filter at the top (reuses existing codebase switcher)
- Search/filter input

### Main Content Area

- Renders the selected document as a readable page
- Chunks displayed in `documentOrder` as sections with their H2 headings
- Each section has a subtle "edit" icon linking to the chunk's edit page (`/chunks/:id/edit`)
- Tags and connections for each chunk shown as small inline badges (collapsed by default)
- Health score badge on each section if available

### Navigation

- Previous/Next document links at page bottom
- Breadcrumb: `Docs > folder > document title`
- Anchor links for each section (heading slug) enabling deep links like `/docs/:id#section-name`

No inline editing — the docs view is a reading experience. Editing happens through the existing chunk edit pages.

---

## What Changes

| Layer | Changes |
|-------|---------|
| DB schema | New `document` table, two new nullable columns on `chunk` |
| API | New `documents` route module |
| Service | New `document-service.ts` with split/sync/render logic |
| Parse | Extend `parse-docs.ts` with H2 splitting function |
| CLI | New `docs` subcommand group |
| Web | New `/docs` route with sidebar index + content renderer |
| Existing code | Chunk API responses include `documentId`/`documentOrder` when present |

All existing chunk, import, and export functionality remains unchanged. Documents are purely additive.
