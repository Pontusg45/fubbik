# fubbik

This file provides context about the project for AI assistants.

## Project Overview

- **Ecosystem**: Typescript

## Tech Stack

- **Runtime**: bun
- **Package Manager**: pnpm

### Frontend

- Framework: tanstack-start
- CSS: tailwind
- UI Library: shadcn-ui
- State: tanstack-store

### Backend

- Framework: elysia
- API Client: elysia eden treaty
- Validation: arktype
- Error Handling: effect (typed errors via Effect.tryPromise, tagged errors)

### Database

- Database: postgres
- ORM: drizzle

### Authentication

- Provider: better-auth

### Additional Features

- Testing: vitest
- AI: vercel-ai
- Embeddings: Ollama (nomic-embed-text) for local vector embeddings
- Logging: winston
- Observability: opentelemetry

## Project Structure

```
fubbik/
├── apps/
│   ├── web/         # Frontend application
│   ├── server/      # Backend API
│   └── cli/         # CLI application
├── packages/
│   ├── api/         # API layer (Elysia routes, Eden types)
│   ├── auth/        # Authentication (Better Auth + Drizzle adapter)
│   ├── config/      # Shared TypeScript config
│   ├── db/          # Database schema (Drizzle ORM)
│   └── env/         # Environment validation (Arktype + t3-env)
```

## Architecture Patterns

### Backend: Repository -> Service -> Route

- **Repositories** (`packages/db/src/repository/`): Return `Effect<T, DatabaseError>`. Pure data access, no business logic.
- **Services** (`packages/api/src/*/service.ts`): Compose repository Effects, add business logic, introduce `NotFoundError`/`AuthError`.
- **Routes** (`packages/api/src/*/routes.ts`): Call `Effect.runPromise(requireSession(ctx).pipe(...))`. Errors propagate to global
  `.onError` handler.
- **Global error handler** (`packages/api/src/index.ts`): Extracts Effect errors from FiberFailure, maps `_tag` to HTTP status codes
  (AuthError->401, NotFoundError->404, DatabaseError->500).

### Frontend: Feature-based Structure

- Route files in `apps/web/src/routes/`
- Feature components in `apps/web/src/features/` (e.g., `features/auth/`)
- Shared UI in `apps/web/src/components/ui/`

## API Documentation

The server exposes a Swagger/OpenAPI endpoint at `/docs` (e.g., `http://localhost:3000/docs`).

## Ollama (Optional)

Required for chunk enrichment (summary, aliases, not_about generation) and semantic search.

- `ollama pull nomic-embed-text` — embedding model
- `ollama pull llama3.2` — generation model for metadata
- `OLLAMA_URL` env var (default: `http://localhost:11434`)
- Without Ollama, all other features work normally

## Environment Variables

- `PORT` — Server port (default: `3000`, validated via `packages/env`)
- `OLLAMA_URL` — Ollama server URL (default: `http://localhost:11434`)

## Common Commands

- `pnpm install` - Install dependencies
- `pnpm dev` - Start development server
- `pnpm build` - Build for production
- `pnpm test` - Run tests
- `pnpm ci` - Run full CI pipeline (type-check, lint, test, build, format check, sherif)
- `pnpm run check-types` - Type-check all packages (uses `tsgo`)
- `pnpm db:push` - Push database schema
- `pnpm db:studio` - Open database UI

## Maintenance

Keep CLAUDE.md updated when:

- Adding/removing dependencies
- Changing project structure
- Adding new features or services
- Modifying build/dev workflows

AI assistants should suggest updates to this file when they notice relevant changes.
