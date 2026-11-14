# Dev commands: full list

Reference for CLAUDE.md. Read when you need a command not listed there (DB, services, ports, kit, review-harness, Caddy).

- `pnpm install` (or `just install`) — install dependencies
- Start the fubbik desktop app (`~/Applications/fubbik.app`, from `just desktop-install`) and the API, its database and the web UI
  (http://127.0.0.1:3000) run with it. Nothing else to start
- `pnpm dev` (= `just dev`) — for working on the server or web app: quit the desktop app (it holds :3000; `kill-dev` refuses to kill it),
  then this frees ports 3000/3001 and starts the Rust API (`fubbik serve --port 3000`, sourcing `.env`) and the web app
  (3001)
- `just start-api` / `pnpm dev:server` — Rust API only (builds, then `exec`s the binary); `just start-web` / `pnpm dev:web` — web app only; `just start-desktop` — the desktop app. `just` with no arguments lists the recipe groups (`just --list` for every recipe)
- `pnpm build` — build the TypeScript packages and web app (turbo)
- `pnpm test` — run the TypeScript tests (turbo)
- `just rust-test` — full Rust suite against the canonical test database (`just rust-test-db-start` / `just rust-test-db-stop`);
  `just rust-test-fast` — `cargo fmt --check` plus the database-free crates
- `pnpm seed` — seed database with sample data (`cargo run --locked -p fubbik -- seed`)
- `pnpm ci` — run full CI pipeline (type-check, lint, test, build, format check, sherif)
- `pnpm run check-types` — type-check all packages (uses `tsgo`)
- `pnpm gen:api` — regenerate `packages/client/src/api-types.ts` from `openapi.json` (refresh that with
  `cargo run -q -p fubbik -- openapi > openapi.json`)
- Schema changes: add a migration to `crates/fubbik-db/migrations/` (there is no `db:push`; migrations apply on connect)
- `pnpm db:start` / `pnpm db:stop` — start/stop the compose Postgres
- `pnpm db:studio` — open database UI (Drizzle Studio)
- `cargo run -q -p review-harness -- list/run/init` — convention checks for AI-written changes; rules in `review-rules.toml`
  (`init` writes a starter file)
- `pnpm kill:server` — kill process on port 3000 (calls `fk port kill`)
- `pnpm kill:web` — kill process on port 3001 (calls `fk port kill`)
- `pnpm kill:all` — kill both (calls `fk port kill`)
- `fk sync` (fast-forward only), `fk push` (asks before pushing), `fk branch <name>`, `fk port kill <port>` (listeners only),
  `fk agents link [--dry-run]` ((re)creates the `.claude/skills/` symlinks from `agents/skills/` and `agents/project/skills/`, and the
  `.claude/commands` link to `agents/commands/`) — built from `crates/kit`, run with `cargo run -q -p kit -- <cmd>`

## Local Development with Caddy

Optional HTTPS via a Caddy reverse proxy: see [docs/guide/operations/local-https.md](docs/guide/operations/local-https.md).

