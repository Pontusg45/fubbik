---
tags:
  - guide
  - plans
  - sessions
description: Implementation plans, sessions, and review workflow
---

# Plans and Implementation Sessions

Plans and sessions help you track implementation work, whether done by humans or AI agents.

## Plans

A plan is an ordered list of steps for implementing a feature, fixing a bug, or completing a migration.

### Creating Plans

**From a template:**

```bash
fubbik plan create "Add user authentication" --template feature-dev
```

Built-in templates: `feature-dev`, `bug-fix`, `migration`, `requirement-standard`, `requirement-detailed`.

**From markdown:** Paste or import an AI-generated plan. Lines starting with `- [ ]` or numbered items become steps.

**From requirements:** Auto-generate a plan from selected requirements.

**In the web UI:** Visit `/plans/new` for template selector, markdown paste mode, bulk step entry, requirement linking, and keyboard shortcuts.

### Working with Steps

Each step has a status: `pending`, `in_progress`, `done`, `skipped`, or `blocked`.

```bash
fubbik plan step-done <plan-id> <step-number>
fubbik plan add-step <plan-id> "Deploy to staging"
```

Steps can be nested, link to requirements and chunks, and have notes.

### Plan Lifecycle

1. **Draft** — initial creation, editing steps
2. **Active** — work in progress
3. **Completed** — all steps done
4. **Archived** — no longer relevant

## Implementation Sessions

Sessions track a focused work period — typically an AI agent implementing a plan.

When a session completes:
1. The linked plan is auto-completed (if all steps are done)
2. Requirement statuses are auto-synced (marking them as passing)
3. A **review brief** is generated summarizing what was done

## Traceability

The full chain: Requirement → Plan Step → Implementation Session → Chunk. View at `/coverage`.
