# Multi-Codebase Support — Design Spec

## Overview

Add the ability to organize fubbik knowledge per-codebase. Chunks can belong to multiple codebases or none (global). The CLI auto-detects the active codebase from the working directory, and the web UI provides a manual switcher.

This spec covers **Phase 1** — the core codebase model, CLI auto-detection, web switcher, and API changes. Phase 2 (tag scoping per-codebase, advanced cross-codebase graph views) is out of scope.

## Data Model

### New `codebase` table

| Column      | Type                  | Notes                                              |
| ----------- | --------------------- | -------------------------------------------------- |
| `id`        | text (PK)             | UUID stored as text, consistent with existing schema |
| `name`      | text                  | Human-friendly name, e.g. "fubbik"                 |
| `remoteUrl` | text (nullable)       | Canonical git remote URL (normalized)              |
| `localPaths`| jsonb                 | Array of known local paths for this codebase       |
| `userId`    | text (FK -> user)     | Owner                                              |
| `createdAt` | timestamp             |                                                    |
| `updatedAt` | timestamp             |                                                    |

**Constraints:**
- Unique on `(userId, remoteUrl)` where `remoteUrl` is not null
- Unique on `(userId, name)`

**Remote URL normalization:** Strip `.git` suffix, trailing slashes, and normalize protocol differences (`git@github.com:user/repo` and `https://github.com/user/repo` resolve to the same canonical form). Normalization is the responsibility of the **service layer** on every write (create and update), before persistence. This ensures the unique constraint catches duplicates regardless of how the URL was submitted.

### New `chunk_codebase` join table

| Column       | Type                    | Notes                                |
| ------------ | ----------------------- | ------------------------------------ |
| `chunkId`    | text (FK -> chunk)      | `ON DELETE CASCADE`                  |
| `codebaseId` | text (FK -> codebase)   | `ON DELETE CASCADE`                  |

**Composite primary key:** `(chunkId, codebaseId)`

**Additional index:** Index on `chunkId` alone to support efficient "find chunks not in any codebase" queries.

**Cascade behavior:** Both FKs use `ON DELETE CASCADE`. Deleting a codebase removes the join rows (unlinking chunks, not deleting them). Deleting a chunk removes its join rows. This is consistent with how `chunkTag` handles cascades.

**Ownership invariant:** The service layer must enforce that `chunk.userId == codebase.userId` before inserting into `chunk_codebase`. This is an application-level check in the codebase service, consistent with how chunk-tag associations are validated.

Chunks with no rows in this table are considered **global** — visible in all codebases.

### Relationship to existing `scope` field

The `scope` JSONB field on chunks remains as-is — it serves a different purpose (arbitrary key-value metadata for filtering). The codebase model is a first-class entity with its own table, not a scope convention. The two coexist independently: `scope` is for ad-hoc metadata, codebases are for organizational grouping. No migration of existing `scope` data is needed.

## CLI Auto-Detection

When a CLI command runs, fubbik resolves the active codebase:

1. Run `git remote get-url origin` (or iterate remotes) to get the remote URL
2. Normalize the URL
3. Look up `codebase` by `remoteUrl` match for the current user
4. If no remote match, fall back to checking `localPaths` against the current working directory. Matching is **exact path equality** — the stored path must match the cwd exactly. Users register the root of their checkout, not subdirectories.
5. If no match, prompt: "This looks like a new codebase. Register it?" — or operate in global mode with a flag

### New CLI commands

- `fubbik codebase add <name>` — register current directory (auto-detects git remote)
- `fubbik codebase add <name> --path /some/dir --remote <url>` — explicit registration
- `fubbik codebase list` — show all registered codebases
- `fubbik codebase remove <name>` — unregister a codebase. Warns the user: "This will unlink N chunks from this codebase. Chunks will not be deleted." Requires confirmation or `--force` flag.
- `fubbik codebase current` — show which codebase is detected in cwd

### Implicit scoping

Once a codebase is detected, commands like `list`, `search`, `add` default to that codebase's chunks.

- `--global` flag shows/creates chunks without codebase association
- `--codebase <name>` overrides auto-detection

When creating a chunk via CLI, it is automatically associated with the detected codebase unless `--global` is passed.

## Web UI

### Codebase switcher

Located in the sidebar/nav. Dropdown showing:
- All registered codebases
- A "Global" option (chunks not associated with any codebase)
- An "All" option (shows everything)

The selected codebase is stored in a URL query param (`?codebase=<id>`) for shareability and bookmarkability.

### Page changes

- **`/chunks`** — filtered to active codebase by default. Global chunks always appear. Codebase badge on chunks belonging to multiple codebases.
- **`/chunks/new`** — pre-selects current codebase. Multi-select to assign to multiple codebases, or leave empty for global.
- **`/chunks/:id`** — shows which codebases the chunk belongs to, editable. Updating codebase associations uses a full replace semantic: send the desired list, service diffs and applies inserts/deletes.
- **`/graph`** — scoped to active codebase by default. Includes: (a) chunks in the active codebase, (b) global chunks connected to codebase chunks, (c) all connections between included chunks. Chunks belonging only to other codebases are excluded. "Show all" toggle for the full graph.
- **`/dashboard`** — stats scoped to active codebase. Summary across all when "All" is selected.
- **`/tags`** — unchanged in Phase 1 (tags remain global).

### New page

- **`/codebases`** — manage codebases (add, rename, remove, view local paths and remote URL).

## API Changes

### New endpoints

| Method   | Path                   | Description                                            |
| -------- | ---------------------- | ------------------------------------------------------ |
| `GET`    | `/codebases`           | List user's codebases                                  |
| `POST`   | `/codebases`           | Create codebase                                        |
| `GET`    | `/codebases/detect`    | Given `remoteUrl` or `localPath`, return matching codebase |
| `PATCH`  | `/codebases/:id`       | Update name, paths, remote URL                         |
| `DELETE` | `/codebases/:id`       | Delete codebase (chunks unlinked, not deleted)         |

**Route ordering:** `/codebases/detect` must be declared before `/codebases/:id` in the route file to avoid the dynamic segment capturing "detect" as an id.

### Modified endpoints

All changes are **additive and backward compatible** — new params are optional.

- `GET /chunks` — new optional `codebaseId` query param. When set, returns chunks in that codebase + global chunks (via `LEFT JOIN chunk_codebase` with `WHERE codebaseId = X OR chunkId NOT IN chunk_codebase`). Without it, returns all chunks.
- `POST /chunks` — new optional `codebaseIds` body field (array of text IDs). Omit for global.
- `PATCH /chunks/:id` — new optional `codebaseIds` field to update associations (full replace semantic).
- `GET /graph` — new optional `codebaseId` param to scope the graph.
- `GET /stats` — new optional `codebaseId` param for scoped stats.
- `GET /chunks/export` — codebase-aware: export includes codebase associations.
- `POST /chunks/import` — can map to a target codebase.

### Backend pattern

Follows existing Repository -> Service -> Route pattern:
- New `codebase` repository in `packages/db/src/repository/`
- New `codebase` service in `packages/api/src/codebases/service.ts`
- New `codebase` routes in `packages/api/src/codebases/routes.ts`
- Chunk repository extended with `findByCodebase` query joining through `chunk_codebase`

## Phase 2 (Future)

Out of scope for this spec, but planned:
- **Tag scoping** — tags can be global or per-codebase via a `tag_codebase` join table
- **Cross-codebase graph views** — visualize connections between codebases
- **Codebase templates** — pre-populate a codebase with common tag types and chunk templates
