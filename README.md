# Fubbik

> **Making your AI less of an Fubbik.**

AI coding agents forget your conventions, make the same mistakes again, and can't tell whether a UI change really works. Fubbik is a
collection of framework tools for working with AI. It gives agents the knowledge they need, checks their output against your rules, and
lets them prove their changes work before they land.

For example, a convention in this repo says UI components use base-ui's `render` prop, not Radix's `asChild`. Without Fubbik, an agent
reaches for `asChild` because most of its training data does. With Fubbik, the agent gets that convention, and the reason for it, as
context before it starts. If it slips anyway, the review harness flags it.

Fubbik is an **opinionated, personal toolkit**, not a general-purpose product. See [Making it yours](#making-it-yours).

---

## How it fits together

The tools form a loop. Each pass leaves the knowledge base a little better for the next agent.

```
        ┌──────────────────────────────────────────────────────────┐
        │                                                          │
        ▼                                                          │
  Knowledge base ──► Context ──► AI writes code ──► Review harness │
  (chunks, plans,    (budgeted,                     (lint against  │
   requirements)      per file/task)                 your rules)   │
                                                         │         │
                                                         ▼         │
                                     GUI tests + Storybook prototypes
                                     (prove it works, review visually)
                                                         │         │
                                                         └─────────┘
                                          new knowledge, flagged staleness
```

## The toolkit

| Tool                          | Status       | Stands alone?                     |
| ----------------------------- | ------------ | --------------------------------- |
| 🧠 Knowledge base             | Available    | Yes. This is the core.            |
| 📋 Planning and coordination  | Available    | Needs the knowledge base          |
| 🔍 Review harness             | Available    | Yes. Standalone Rust binary       |
| 🧪 GUI test framework         | Available    | Yes. Plain Playwright             |
| 🎨 Storybook prototyping      | Planned      | Needs only the web app            |
| ⚡ `fk` terminal CLI          | Available    | Yes. Standalone Rust binary       |
| 🔌 MCP, CLI, web, VS Code     | Available    | Interfaces to the knowledge base  |

### 🧠 Knowledge base

- **Knowledge chunks**: short, typed records of conventions, architecture decisions and runbooks. Each one is linked to the files it covers
  and can record why a decision was made, what the alternatives were and what follows from it. Your AI gets the reasoning along with the rule.
- **Context on demand**: picks the chunks relevant to a file, plan or concept and fits them into a token budget. It can also generate
  `CLAUDE.md` files and save frozen snapshots.
- **Freshness checks**: chunks get flagged when their linked code changes or they get old, so stale knowledge doesn't mislead your AI.

### 📋 Planning and coordination

- **Plans and specs**: plans with tasks and dependencies, BDD-style requirements (Given/When/Then), and behavioral matrices that tie
  expected behavior to code and test results. This gives agents a clear definition of done.
- **Agent coordination boards**: shared boards where a lead agent and its sub-agents claim tasks, post notes and questions, and hand off
  work without stepping on each other.

### 🔍 Review harness

A linting and review layer that checks AI-written changes against your project's rules before they land. It catches the mistakes an AI
tends to make over and over and turns your conventions into checks it can't skip. It lives in `crates/review-harness`: implement the
`Check` trait, register it in `src/checks/mod.rs`, or just write regex rules in `review-rules.toml`, and run
`cargo run -p review-harness -- run`. See the [review harness README](crates/review-harness/README.md). This repo's own rules include:

- No Radix `asChild`. Use the base-ui `render` prop.
- No `arktype` outside the web sign-in/sign-up forms and `packages/env`.

### Use the tools without forking

`fk` and the review harness are standalone Rust binaries. Install them straight from the repo, with no database or server:

```bash
cargo install --git https://github.com/Pontusg45/fubbik kit             # installs `fk`
cargo install --git https://github.com/Pontusg45/fubbik review-harness
```

See the [`fk` README](crates/kit/README.md) and the [review harness README](crates/review-harness/README.md).

### 🧪 GUI test framework

An end-to-end testing setup built on Playwright, organized as screen objects (`apps/web/e2e/support/screens/`) with shared fixtures. It lets
an AI prove a UI change works in the real app, not just in unit tests. Run it with `pnpm run test:e2e`.

### 🎨 Storybook prototyping _(planned)_

A sandbox where an AI can build and try out components on their own before wiring them into the app. You review the result visually first,
which keeps throwaway experiments out of your codebase.

### ⚡ `fk` terminal CLI

A small Rust binary (`crates/kit`) for the git and dev chores an AI agent runs most. Every command takes a global `--json` flag.

- `fk sync [--dry-run]`: fast-forward only. It refuses when the branch has diverged from its upstream.
- `fk push [--yes]`: lists the commits, then asks before pushing. With no terminal it refuses unless `--yes` is passed.
- `fk branch <name> [--from <base>]`: creates a branch, and refuses when the tree has uncommitted changes.
- `fk port kill <port>... [--dry-run]`: kills only processes listening on the port. A free port is not an error.

### 🔌 Interfaces

- **MCP server** (`fubbik mcp`): tools for AI agents, covering knowledge, context, requirements, plans, coordination boards and matrices.
  See the [MCP guide](docs/guide/operations/integrations/mcp.md).
- **CLI**: pipeable and scriptable, with `--json` output, git hooks and `CLAUDE.md` sync.
- **Web UI**: dashboard, knowledge graph, editors, plans and matrices.
- **VS Code extension**: shows the chunks relevant to the file you have open.

---

## Quickstart

Requires PostgreSQL 18 with pgvector, plus Rust, pnpm and bun. Ollama is optional and powers AI enrichment and semantic search.

```bash
pnpm install
# set DATABASE_URL in .env (copy .env.example)
pnpm dev     # web on :3001, API on :3000, applies migrations
pnpm seed    # optional: loads the fubbik demo knowledge base
```

Forking? `cargo run -p fubbik -- seed --scenario minimal` loads a neutral starter instead of the fubbik demo.

Connect it to Claude Code:

```bash
claude mcp add fubbik -e FUBBIK_SERVER_URL=http://localhost:3000 -- fubbik mcp
```

Or run the whole stack in Docker: `docker compose -f docker-compose.selfhost.yml up -d` (see [Self-hosting](#self-hosting)).

---

## Making it yours

Fubbik is shaped around one developer's way of working with AI. **If it interests you, the recommended way to use it is to fork this repo
and adapt it to your own workflow.** Keep the tools that help, remove the ones that don't, and change the conventions to fit your team and
stack.

Follow the [forking checklist](docs/forking.md) step by step. For what is safe to build on, see [extending fubbik](docs/extending.md).

Good places to start in a fork:

| What                   | Where                                                    |
| ---------------------- | -------------------------------------------------------- |
| AI instructions        | `CLAUDE.md` and `agents/`                                |
| Starter knowledge      | the seed data (`pnpm seed`, `crates/fubbik`)             |
| Chunk templates        | the built-in templates, or your own via `/templates`     |
| GUI test screens       | `apps/web/e2e/support/screens/`                          |
| Review rules           | `review-rules.toml` (compiled checks in `crates/review-harness/src/checks/`) |

**What it costs to run:** a Postgres 18 database with pgvector, and optionally Ollama. The Rust backend in `crates/` is the only one;
the retired TypeScript backend was removed (it is in git history).

Fubbik is MIT-licensed. Forks are the expected way to use it. Issues and ideas are welcome, but don't expect it to grow into a
general-purpose product.

---

## Getting Started

First, install the dependencies:

```bash
pnpm install
```

## Database Setup

This project uses PostgreSQL. The Rust server and its embedded SQLx migrations
are the schema authority. The sample-data seed runs through `fubbik seed` in
Rust. Drizzle remains for read-only Studio inspection; do not generate or apply schema
changes through Drizzle.

When upgrading an existing database from the former Node/Drizzle server,
startup recognizes Drizzle's migration history, repairs the known five-table
behavior-matrix omission in that migration set, and adopts the resulting
verified 58-table schema as SQLx migration 1 before applying later migrations.
This preserves existing data. Any other missing table—or a schema without
recognized Drizzle bookkeeping—fails without changing migration history; back
up and finish the legacy migrations before retrying.

1. Make sure you have a PostgreSQL database set up.
2. Update your `.env` file (copy `.env.example`) with your PostgreSQL connection details.

3. Run the development server (it applies pending migrations automatically):

```bash
pnpm run dev
```

Open [http://localhost:3001](http://localhost:3001) in your browser to see the web application. The API is running at
[http://localhost:3000](http://localhost:3000). API documentation (Swagger) is available at
[http://localhost:3000/api/docs](http://localhost:3000/api/docs).

The server port can be configured via the `PORT` environment variable (default: `3000`).

### Rust CLI

The `fubbik` binary now covers the primary server-backed workflows directly:

```bash
fubbik add "Architecture note" --content "..."
fubbik space list
fubbik tag list
fubbik link <source-id> <target-id> --relation supports
fubbik req list --status failing
fubbik docs import README.md
fubbik enrich --all
fubbik stale list
fubbik stats
```

These workflows support the global `--json` and `--quiet` output modes. The
compatibility aliases `codebase`, `tags`, and `requirements` are also accepted.

CLI sessions are scoped to the server URL. Sign in with `fubbik auth login
you@example.com` (or pipe a password with `--password-stdin`), inspect the
active identity with `fubbik auth whoami`, and revoke it with `fubbik auth
logout`. Session tokens are stored in a private file under
`$XDG_CONFIG_HOME/fubbik/sessions.json` or `~/.config/fubbik/sessions.json`;
`FUBBIK_SESSION` supplies a token for noninteractive jobs. Login requires HTTPS
except when connecting to localhost.

Use `fubbik task add "Ship release" --priority high` to create a queue task.
`fubbik task list --status completed` shows queue history; `task claim` and
`task done` take the task's plan ID. Chunk listing and search accept `--space`,
`--offset`, `--limit` (up to 100 per page), and `--all` to fetch every page.

For large JSONL imports, run `fubbik bulk-add --file chunks.jsonl --dry-run`
first, then `fubbik bulk-add --file chunks.jsonl --report import-report.jsonl`.
If a run is interrupted, repeat it with `--resume import-report.jsonl` to skip
successful lines. The resume report records line numbers, SHA-256 fingerprints,
and created IDs; changing a successful line causes the resume to stop. JSON and
Markdown `fubbik import` also accept `--dry-run`. With `--json`, command results
go to stdout and errors go to stderr as JSON.

### Self-hosting

Start the Rust server and PostgreSQL stack with:

```bash
docker compose -f docker-compose.selfhost.yml up -d
```

Open http://localhost:4717 for the web UI. The same port serves `/api/*`.
Set `FUBBIK_API_PORT` to publish a different port.

For a first-run demo dataset, set `SEED_DATABASE=true`. Seeding runs as a
one-shot tool after the Rust server has applied migrations; Node/Bun is absent
from both runtime images.

### Self-hosted backups

The Compose stack keeps PostgreSQL data in the `fubbik_data` volume. Create a
portable backup before upgrades:

```bash
docker compose -f docker-compose.selfhost.yml exec -T db pg_dump -U fubbik -d fubbik -Fc > fubbik.backup
```

Verify that the backup can be restored into a disposable database:

```bash
docker compose -f docker-compose.selfhost.yml exec -T db createdb -U fubbik fubbik_restore_check
docker compose -f docker-compose.selfhost.yml exec -T db pg_restore -U fubbik -d fubbik_restore_check --clean --if-exists < fubbik.backup
docker compose -f docker-compose.selfhost.yml exec -T db dropdb -U fubbik fubbik_restore_check
```

Run the restore check periodically; a backup that has never been restored is
not yet a verified backup.

See [self-hosted operations](docs/operations/self-hosted.md) for upgrades,
rollback, log collection, seeding, and release archives.

## Project Structure

```
fubbik/
├── apps/
│   ├── web/         # Frontend (TanStack Router + React, Vite SPA)
│   └── vscode/      # VS Code extension
├── packages/
│   ├── client/      # Generated OpenAPI client and wire contracts
│   ├── db/          # Typed seed adapter and legacy schema reference
│   ├── env/         # Environment validation
│   └── config/      # Shared TypeScript config
├── crates/          # Rust API, database, CLI and core libraries
│   ├── kit/         # `fk` terminal CLI
│   └── review-harness/  # Review linting tool (rules in `review-rules.toml`)
├── agents/          # Authored skills, commands and MCP config for AI agents
├── scripts/
│   ├── test/        # Rust test and test-database scripts
│   └── release/     # Release packaging and smoke tests
└── docs/
```

## Test conventions

Mark test bodies with `// Given`, `// When`, and `// Then` comments for setup, the operation under test, and its assertions. This applies to Rust, TypeScript, JavaScript, browser tests, and compile-time contracts. When an assertion also evaluates the operation, keep the When and Then comments together above it. Repeat the action and outcome markers for multi-step scenarios.

## Available Scripts

- `pnpm run dev`: Start the web app and Rust API
- `pnpm run build`: Build all applications
- `pnpm run dev:web`: Start only the web application
- `pnpm run dev:server`: Start only the Rust server
- `pnpm run check-types`: Type-check all packages (uses `tsgo`)
- `pnpm run db:studio`: Open database studio UI
- `just rust-test`: Start the canonical PG18/vector/AGE adapter, run formatting, strict Clippy, migration checks, and the complete Rust suite, then stop it
- `just rust-test-fast`: Run database-free Rust libraries and CLI HTTP contract tests
- `pnpm test`: Run workspace tests after starting the [database test adapter](packages/db/README.md)
- `pnpm run ci`: Run the TypeScript CI pipeline
- `pnpm run test:e2e`: Run the browser critical-path suite

---

## License

MIT
