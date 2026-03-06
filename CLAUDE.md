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
