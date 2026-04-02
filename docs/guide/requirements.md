---
tags:
  - guide
  - requirements
description: BDD requirements, plans, and implementation tracking
---

# Requirements and Plans

Fubbik includes a requirements system for defining what your software should do and tracking its implementation.

## BDD Requirements

Write structured requirements in Given/When/Then (BDD) format:

```
Given a user is logged in
And they have chunks in their knowledge base
When they visit the dashboard
Then they see their chunk count
And they see recent activity
```

Requirements have a **priority** (must, should, could, won't) and a **status** (passing, failing, untested).

## Controlled Vocabulary

Each codebase has a controlled vocabulary — a dictionary of valid words organized by category:

- **Actors** — user, admin, system
- **Actions** — clicks, creates, deletes
- **Targets** — chunk, codebase, tag
- **Outcomes** — sees, receives, is redirected
- **States** — logged in, on the dashboard

The step builder at `/requirements/new` validates your text in real-time and highlights unknown words. Manage vocabulary at `/vocabulary`.

## Plans

Plans track implementation work as ordered steps. Create plans from:

- **Templates** — feature-dev, bug-fix, migration, requirement-standard
- **Requirements** — auto-generate a plan from selected requirements
- **Markdown** — paste an AI-generated plan and it'll be parsed into steps
- **Manual entry** — add steps one by one

Each plan step can link to a requirement for traceability and has its own status: pending, in_progress, done, skipped, or blocked.

## Implementation Sessions

Sessions track AI implementation work. When an AI agent works on a plan:

1. Begin a session linked to the plan
2. Reference chunks used during implementation
3. Track assumptions and knowledge gaps
4. Mark requirements addressed
5. Complete the session to auto-sync statuses

Sessions generate a **review brief** summarizing what was done, what requirements were addressed, and what assumptions were made.

## Export Formats

Requirements export to three formats:

- **Gherkin** (`.feature`) — for Cucumber-style test runners
- **Vitest** (`.test.ts`) — TypeScript test scaffolds
- **Markdown** — checklist format

## Traceability

The traceability view at `/coverage` shows the full chain: requirements > plan steps > implementation sessions > chunks. This helps you verify that every requirement has been addressed and every chunk links back to a reason for existing.
