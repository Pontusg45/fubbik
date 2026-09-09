# Contributing to Fubbik

## Prerequisites

- Node.js v22+
- Bun runtime
- pnpm package manager
- Docker (the test adapter supplies PostgreSQL 18, pgvector, AGE, and ICU)
- (Optional) Ollama for AI features

## Setup

1. Clone the repo
2. `pnpm install`
3. Copy `apps/server/.env.example` to `apps/server/.env` and fill in values
4. `just rust-test-db-start` — start the canonical database; the Rust server applies migrations
5. `pnpm dev` — starts web (port 3001) + API (port 3000)

## Project Structure

```
fubbik/
├── apps/
│   ├── web/         # Frontend (TanStack Start + React)
│   ├── server/      # Retired TypeScript reference
│   ├── cli/         # Legacy TypeScript CLI workflows
│   └── vscode/      # VS Code extension
├── packages/
│   ├── api/         # Retired Elysia reference
│   ├── auth/        # Retired Better Auth reference
│   ├── client/      # Generated OpenAPI client and wire contracts
│   ├── db/          # Typed seed adapter and legacy schema reference
│   ├── env/         # Environment validation
│   ├── mcp/         # MCP server for AI agents
│   └── config/      # Shared TypeScript config
```

## Architecture

The active Rust backend follows **repository → service → route** under
`crates/fubbik-db` and `crates/fubbik-api`. SQLx migrations are the sole schema
authority. The old TypeScript backend directories remain as historical porting
references but are excluded from the workspace and CI.

- **Repository** (`crates/fubbik-db/src/repo/`): tenant-scoped PostgreSQL behavior
- **Service** (`crates/fubbik-api/src/*/service.rs`): domain orchestration
- **Route** (`crates/fubbik-api/src/*/routes.rs`): Axum HTTP interface

## Adding a New Feature

### 1. Database schema

Add a forward-only SQLx migration under `crates/fubbik-db/migrations/`.

### 2. Repository

Create `crates/fubbik-db/src/repo/your_feature.rs` and export it from `repo/mod.rs`.

### 3. Service

Create `crates/fubbik-api/src/your_feature/service.rs`.

### 4. Routes

Create `crates/fubbik-api/src/your_feature/routes.rs` and register its router.

### 5. Web UI

Create route at `apps/web/src/routes/your-feature.tsx`.

### 6. Verify

Run the focused test, then `just rust-test`.

## Common Commands

| Command                | Description             |
| ---------------------- | ----------------------- |
| `pnpm dev`             | Start dev server        |
| `pnpm build`           | Build for production    |
| `pnpm test`            | Run tests               |
| `pnpm run check-types` | Type-check all packages |
| `pnpm ci`              | Full CI pipeline        |
| `pnpm db:studio`       | Open Drizzle Studio     |
| `just rust-test`       | Full Rust suite on PG18 + AGE |
| `pnpm kill:all`        | Free ports 3000 + 3001  |
| `pnpm service:start`   | Start via launchd       |

## Code Style

- TypeScript strict mode
- Formatting: `pnpm fmt`
- Linting: `pnpm lint`
- All database PKs use `text` type (UUID as text)
- SQLx migrations are forward-only and schema-qualified
- Axum extractors validate the HTTP interface
