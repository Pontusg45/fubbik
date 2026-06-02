# Domain-Agnostic Database

**Status:** Draft **Date:** 2026-06-02

## Goal

Decouple the database schema from its code-knowledge bias so fubbik works equally well for non-code domains (research notes, personal wikis,
product specs, recipes, etc.) without breaking existing code-knowledge usage.

## Non-goals

- Renaming `chunk` itself.
- Reworking the `document` table's shape or semantics (the `document.codebase_id` → `document.space_id` rename is a follow-on of the
  codebase rename, not a redesign).
- Adding UI for selecting space `kind` on space-create.
- Migrating `staleness.reason='file_changed'` into a code-only flag.

## Design

Three coordinated schema changes:

1. `codebase` → `space` with a `kind` discriminator; git-specific fields move to a 1:1 side-table.
2. New `chunk_attachment` table generalises `chunk_applies_to` and `chunk_file_ref`, and adds room for new external-reference kinds.
3. `chunk.documentId` / `documentOrder` / `isEntryPoint` stay as direct columns (FK + ordering constraints make a JSONB attachment a worse
   fit).

### 1. `space` (replaces `codebase`)

```
space
├── id              text PK
├── name            text NOT NULL
├── kind            text NOT NULL FK → space_kind
├── description     text
├── user_id         text FK → user (cascade)
├── created_at      timestamp
└── updated_at      timestamp
unique(user_id, name)
index(kind)
index(user_id)

space_kind                       -- lookup, seeded
├── id              text PK       -- 'code' | 'wiki' | 'research' | 'notes' | ...
├── label           text
├── description     text
├── icon            text
└── builtin         boolean

space_code_metadata              -- 1:1, only present when kind='code'
├── space_id        text PK FK → space (cascade)
├── user_id         text NOT NULL  -- denormalised for the partial unique below
├── remote_url      text
└── local_paths     jsonb string[] default []
unique(user_id, remote_url) WHERE remote_url IS NOT NULL
```

Renames that follow from this:

| Old                              | New                        |
| -------------------------------- | -------------------------- |
| `chunk_codebase`                 | `chunk_space`              |
| `chunk_codebase.codebase_id`     | `chunk_space.space_id`     |
| `workspace_codebase`             | `workspace_space`          |
| `workspace_codebase.codebase_id` | `workspace_space.space_id` |
| `document.codebase_id`           | `document.space_id`        |
| `staleness_scan.codebase_id`     | `staleness_scan.space_id`  |

`staleness_scan.last_scanned_commit_sha` stays on `staleness_scan` for now (an `IS NOT NULL` check is enough to skip non-code spaces);
revisit if other code-only scan state appears.

`space_kind` seed: `code`, `wiki`, `notes`, `research` (all `builtin = true`; user-defined kinds can be added later).

### 2. `chunk_attachment`

```
chunk_attachment
├── id              text PK
├── chunk_id        text FK → chunk (cascade)
├── kind            text NOT NULL FK → attachment_kind
├── data            jsonb NOT NULL    -- shape varies per kind, validated in service layer
├── label           text              -- optional human-readable label
├── created_at      timestamp
└── updated_at      timestamp
index(chunk_id)
index(kind)

attachment_kind                  -- lookup, seeded
├── id              text PK       -- 'glob' | 'file_ref' | 'url' | 'external_id'
├── label           text
├── description     text
└── builtin         boolean
```

Per-kind `data` shapes:

```ts
glob:        { pattern: string }
file_ref:    { path: string, symbol?: string, refKind?: 'file' | 'function' | 'class' }
url:         { url: string, title?: string }
external_id: { provider: string, id: string }
```

Validation lives in `packages/api/src/chunk-attachment/service.ts` via Elysia `t` schemas keyed on `kind`.

`attachment_kind` seed: `glob`, `file_ref`, `url`, `external_id` (all builtin).

### 3. Chunks stay as-is

`chunk.documentId` / `documentOrder` / `isEntryPoint` remain direct columns because `document` is a fubbik-internal entity with FK integrity
and a `unique(document_id, document_order)` constraint that would be lost in JSONB.

`rationale` / `alternatives` / `consequences` stay as nullable columns — small dead weight on non-decision chunks, but worth it to keep the
existing structure stable.

`summary` / `aliases` / `notAbout` / `embedding` / `scope` are already domain-neutral and need no change.

## Migration

### Phase A — `codebase` → `space`

1. Create `space_kind`, `space`, `space_code_metadata` tables. Seed `space_kind`.
2. Copy every `codebase` row into `space` with `kind='code'` (same `id`, preserving FKs). Insert matching `space_code_metadata` rows with
   `remote_url` and `local_paths`.
3. Rename FK columns in `chunk_codebase`, `workspace_codebase`, `document`, `staleness_scan` (or recreate tables with the new names — see
   Open questions).
4. Drop `codebase`.
5. App-layer rename across repository, service, routes, Eden client, CLI, frontend feature folder, VS Code extension endpoints, seed data,
   tests.

### Phase B — `chunk_attachment` (additive then collapse)

1. Create `attachment_kind` and `chunk_attachment`. Seed `attachment_kind`.
2. Backfill from existing tables:
    - `chunk_applies_to` row → `chunk_attachment(kind='glob', data={pattern})`
    - `chunk_file_ref` row → `chunk_attachment(kind='file_ref', data={path, symbol, refKind})`
3. Update services to read/write `chunk_attachment`.
4. Drop `chunk_applies_to` and `chunk_file_ref`.
5. Surface the new `url` and `external_id` kinds in UI / CLI (additive, no data migration).

## Touch-points

- `packages/db` — schema files, migrations, repository layer.
- `packages/api/src/{codebase,workspace,context,staleness,chunk}/` — services, routes; new `chunk-attachment/` module.
- `packages/api` Eden types and `/api/codebases` → `/api/spaces` route paths. Add `/api/chunks/:id/attachments`.
- CLI: rename `fubbik codebase` → `fubbik space`; update every command flag `--codebase` to `--space`.
- Frontend: rename `features/codebases` → `features/spaces`; update the nav switcher, codebase pages, applies-to + file-refs panels on chunk
  detail, graph styling that keys on codebase.
- VS Code extension: update HTTP endpoint paths.
- Seed data (`pnpm seed`).
- Tests across all of the above.

## Open questions

- **Rename vs recreate tables in Phase A.** Renaming columns is cleaner but Drizzle migrations sometimes prefer drop-and-recreate; decide
  during plan-writing.
- **Backwards-compat for `/api/codebases` endpoints.** External callers (the VS Code extension) hit these. Either ship both endpoints for
  one release or do a hard cutover and bump the extension in lockstep.
- **CLI command alias.** Should `fubbik codebase ...` continue to work as a deprecated alias of `fubbik space ...` for a release?

## Risks

- The Phase A rename touches a lot of files. A single missed reference can produce runtime errors that type-checking won't catch (template
  strings, Eden URL paths, CLI command lookups).
- Backfilling `chunk_attachment` from two source tables needs careful idempotency so a partial run can be re-run safely.
- `space_code_metadata.user_id` is denormalised for the partial unique index — must be kept in sync with `space.user_id` (trigger or
  service-layer guard).

## Out of scope (future work)

- Per-`kind` typed columns on `space` (currently all custom state lives in `space_code_metadata` or future `space_*_metadata` tables; a
  generic JSONB `metadata` could be added later if non-code kinds need structured state).
- Renaming `chunk` itself.
- User-defined entity types beyond chunks.
- Custom typed fields per chunk type.
