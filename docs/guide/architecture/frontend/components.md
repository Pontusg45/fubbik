---
tags:
  - guide
  - architecture
  - frontend
  - components
description: Feature structure, shared UI, and base-ui component patterns
---

# Component Patterns

## Feature-Based Structure

Domain-specific components live in `apps/web/src/features/`:

- `features/auth/` — login, session management
- `features/graph/` — graph visualization components
- `features/codebases/` — codebase switcher, management
- `features/plans/` — plan creation, detail, task management
- `features/chunks/` — chunk list, detail, editing

## Shared UI

Reusable components in `apps/web/src/components/ui/`:

- Built on **@base-ui/react** (NOT Radix)
- Uses the `render` prop pattern (NOT `asChild`)
- `PageContainer`, `PageHeader`, `PageLoading`, `PageEmpty` for consistent page layouts
- `DropdownMenuSeparator` and `DropdownMenuLabel` use plain HTML elements

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | TanStack Start (SSR) |
| Styling | Tailwind CSS |
| UI Library | shadcn-ui (base-ui) |
| API Client | Eden treaty |
| State/Data | React Query |
