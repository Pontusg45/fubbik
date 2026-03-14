# Contributing to Fubbik

## Prerequisites

- Node.js v22+
- Bun runtime
- pnpm package manager
- PostgreSQL with pgvector extension
- (Optional) Ollama for AI features

## Setup

1. Clone the repo
2. `pnpm install`
3. Copy `apps/server/.env.example` to `apps/server/.env` and fill in values
4. `pnpm db:push` — create database tables
5. `pnpm dev` — starts web (port 3001) + API (port 3000)

## Project Structure

```
fubbik/
├── apps/
│   ├── web/         # Frontend (TanStack Start + React)
│   ├── server/      # API server (Elysia)
│   ├── cli/         # CLI (Commander.js)
│   └── vscode/      # VS Code extension
├── packages/
│   ├── api/         # API routes + services (Elysia + Effect)
│   ├── auth/        # Authentication (Better Auth)
│   ├── db/          # Database schema + repositories (Drizzle ORM)
│   ├── env/         # Environment validation
│   ├── mcp/         # MCP server for AI agents
│   └── config/      # Shared TypeScript config
```

## Architecture

Backend follows **Repository -> Service -> Route**:
- **Repository** (`packages/db/src/repository/`): Pure data access, returns `Effect<T, DatabaseError>`
- **Service** (`packages/api/src/*/service.ts`): Business logic, composes Effects
- **Route** (`packages/api/src/*/routes.ts`): HTTP layer, `requireSession(ctx).pipe(...)`

## Adding a New Feature

### 1. Database schema
Create `packages/db/src/schema/your-feature.ts`, export from `schema/index.ts`.

### 2. Repository
Create `packages/db/src/repository/your-feature.ts`, export from `repository/index.ts`.

### 3. Service
Create `packages/api/src/your-feature/service.ts`.

### 4. Routes
Create `packages/api/src/your-feature/routes.ts`, register in `packages/api/src/index.ts`.

### 5. Web UI
Create route at `apps/web/src/routes/your-feature.tsx`.

### 6. Push schema
Run `pnpm db:push`.

## Common Commands

| Command | Description |
|---------|-------------|
| `pnpm dev` | Start dev server |
| `pnpm build` | Build for production |
| `pnpm test` | Run tests |
| `pnpm run check-types` | Type-check all packages |
| `pnpm ci` | Full CI pipeline |
| `pnpm db:push` | Push schema changes |
| `pnpm db:studio` | Open Drizzle Studio |
| `pnpm kill:all` | Free ports 3000 + 3001 |
| `pnpm service:start` | Start via launchd |

## Code Style

- TypeScript strict mode
- Formatting: `pnpm fmt`
- Linting: `pnpm lint`
- All database PKs use `text` type (UUID as text)
- Effect for typed error handling in repositories/services
- Elysia `t.Object()` for route validation
