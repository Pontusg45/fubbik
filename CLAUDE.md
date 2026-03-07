# fubbik

This file provides context about the project for AI assistants.

## Project Overview

- **Ecosystem**: Typescript

## Tech Stack

- **Runtime**: bun
- **Package Manager**: bun

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

## Environment Variables

- `PORT` — Server port (default: `3000`, validated via `packages/env`)

## Common Commands

- `bun install` - Install dependencies
- `bun dev` - Start development server
- `bun build` - Build for production
- `bun test` - Run tests
- `bun ci` - Run full CI pipeline (type-check, lint, test, build, format check, sherif)
- `bun run check-types` - Type-check all packages (uses `tsgo`)
- `bun db:push` - Push database schema
- `bun db:studio` - Open database UI

## Maintenance

Keep CLAUDE.md updated when:

- Adding/removing dependencies
- Changing project structure
- Adding new features or services
- Modifying build/dev workflows

AI assistants should suggest updates to this file when they notice relevant changes.
