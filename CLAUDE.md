# fubbik

Local-first knowledge framework for storing, navigating and evolving structured knowledge about codebases ("chunks", connections, plans,
requirements, behavior matrices). Used by humans (web UI, graph) and machines (CLI, API, MCP server, VS Code extension).

> **New to this codebase?** Use the `fubbik-dev` skill (`agents/project/skills/fubbik-dev/`): where code goes in the Rust crates and the
> web app, the patterns to copy, the generated files to regenerate, and known pitfalls.

## Hard rules

- **The repo is public (github Pontusg45/fubbik). Always ask before pushing.** Never run `git push` (or force-push) without explicit
  user confirmation.
- Schema changes: add a migration to `crates/fubbik-db/migrations/` (sqlx, applied on connect; there is no `db:push`). `packages/db`
  Drizzle is a read-only reference schema and does not own the schema.
- Do not hand-edit generated files: `openapi.json` (`cargo run -q -p fubbik -- openapi > openapi.json`), then
  `packages/client/src/api-types.ts` (`pnpm gen:api`). Regenerate after changing routes/DTOs.
- UI is shadcn on **@base-ui/react**: use the `render` prop, NOT Radix `asChild`. `DropdownMenuSeparator`/`DropdownMenuLabel` are plain
  HTML elements, not base-ui primitives.
- Backend layering: repository (`fubbik-db/src/repo`) -> service -> route (`fubbik-api/src/<domain>/`). `fubbik-cli` and `fubbik-mcp` are
  HTTP clients of the API; they must not depend on `fubbik-db` or `fubbik-api`. Errors are `AppError`, mapped to HTTP by `ApiError`.
- Optional feature groups (plans, requirements, matrices, coordination, learning_paths, overlays, ai) are gated by `fubbik.toml`;
  a new gated route or background job must respect the switch (see `docs/agents/feature-switches.md`).
- Editing a chunk's title/content triggers a full re-enrich (summary, aliases, not_about are overwritten) when Ollama is up.
- Unless `NODE_ENV=production` (or `FUBBIK_IMPLICIT_DEV_SESSION=true`), the server serves unauthenticated requests as the dev user.
- Package manager is pnpm (TS) + cargo (Rust workspace `crates/*`). Task runner is `just` (`Justfile`); root `package.json` scripts
  delegate to it or to turbo. Lint/format is oxlint/oxfmt; type-check is `tsgo`.

## Stack

- Backend: one Rust binary `fubbik` (`crates/fubbik`) = server + CLI + MCP server. axum + utoipa (OpenAPI), sqlx, Postgres 18
  (pgvector, pg_trgm, Apache AGE), Ollama only for AI (`crates/fubbik-ai`).
- Web: TanStack Router client-rendered SPA on Vite (`apps/web`, dev port 3001, proxies `/api` to the API on 3000), tailwind, shadcn,
  @xyflow/react. API client generated into `packages/client`; `unwrapEden` helper names are legacy (Eden is no longer used).
- Auth: Rust server implements better-auth-compatible endpoints and verifies better-auth session cookies; web uses the better-auth client.

## Layout

```
apps/web/        Vite SPA (routes/, features/, components/ui/)
apps/vscode/     VS Code extension (standalone, HTTP only)
apps/desktop/    GPUI desktop app (own cargo workspace; runs the API in-process on its own PostgreSQL)
packages/        client (generated OpenAPI client), db (Drizzle ref + scripts), env (arktype), config
crates/          fubbik (binary), fubbik-api, fubbik-db, fubbik-core, fubbik-contracts, fubbik-config, fubbik-ai,
                 fubbik-cli, fubbik-mcp, kit (`fk`), review-harness
agents/          authored skills, commands, MCP config for AI agents (.claude/skills and .claude/commands symlink here)
scripts/         test/ and release/ scripts
docs/            guide/, adr/, operations/, plans/, superpowers/, agents/ (reference docs for agents)
```

## Commands

- `pnpm install` (or `just install`)
- Run: the desktop app (`~/Applications/fubbik.app`) already serves API + DB + web UI on http://127.0.0.1:3000. For server/web work,
  quit it (it holds :3000), then `pnpm dev` (= `just dev`): frees ports 3000/3001, starts the Rust API and the web app (3001).
  Individually: `just start-api` / `pnpm dev:server`, `just start-web` / `pnpm dev:web`, `just start-desktop`. `just --list` for all recipes.
- Build: `pnpm build` (turbo; TS packages and web app); `just build`
- Test (TS): `pnpm test` (turbo, vitest); web e2e: `pnpm test:e2e`
- Test (Rust): `just rust-test` (full suite against the test DB; `just rust-test-db-start` / `just rust-test-db-stop`),
  `just rust-test-fast` (`cargo fmt --check` plus DB-free crates), `just test-changed` (changed crates plus reverse deps)
- Lint: `pnpm lint` (`pnpm lint:fix` to fix); Format: `pnpm fmt` (write) / `pnpm fmt:check`; Workspace repo hygiene: `pnpm sherif`
- Typecheck: `pnpm run check-types`
- Full CI locally: `pnpm ci` (check-types, lint, test, build, fmt:check, sherif) or `just ci` (also starts the Rust test DB);
  `pnpm ci:fix` auto-fixes
- Seed sample data: `pnpm seed` (`cargo run --locked -p fubbik -- seed`)
- Run the CLI from source: `cargo run -q -p fubbik -- <command>` (`--help` is the reference)
- Free ports: `pnpm kill:all` (also `kill:server` / `kill:web`)
- Convention checks for AI-written changes: `cargo run -q -p review-harness -- run` (rules in `review-rules.toml`)
- Env: personal settings in `.env` (see `docs/guide/getting-started/configuration/environment.md`); required for server/seed:
  `DATABASE_URL`, `BETTER_AUTH_SECRET` (min 32 chars)

## Further docs (`docs/agents/`)

- `architecture.md` - full tech stack, Repository->Service->Route details, error mapping, frontend structure, context pipeline, MCP
  server, VS Code extension. Read before structural backend/frontend work or MCP/context changes.
- `domain-model.md` - chunks, spaces/workspaces, connections, templates, plans, coordination boards, requirements, matrices, features
  (overlays), staleness, health scores, scope keys. Read when touching these entities or their tables.
- `feature-switches.md` - `fubbik.toml` switch semantics and `/api/capabilities`. Read when adding a gated feature, route or job.
- `api-and-web.md` - endpoint map and web route list. Read when adding or changing routes or pages.
- `cli-reference.md` - `fubbik` CLI command overview. Read when adding or changing CLI commands.
- `seed-ollama-env.md` - seed scenarios/modules, Ollama behavior and failure modes, all env vars. Read for seed, AI/embedding or config work.
- `dev-commands.md` - remaining commands (db, launchd services, `fk` kit tool, Caddy). Read when you need one not listed above.
- Existing: `docs/guide/` (user guide, incl. `operations/local-https.md`), `docs/adr/` (decisions), `apps/desktop/README.md` (built-in
  server, MCP), `agents/README.md` (agent skills/commands).

## Maintenance

Update this file (and the matching `docs/agents/` file) when project structure, build/dev workflow or hard rules change. Keep it short:
detail belongs in `docs/agents/`.
