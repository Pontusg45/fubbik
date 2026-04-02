---
tags:
  - guide
  - plans
  - sessions
description: Implementation plans and AI session tracking
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

**From markdown:**

Paste or import an AI-generated plan:

```bash
fubbik plan import plan.md
```

The markdown is parsed into individual steps. Each line starting with `- [ ]` or a numbered item becomes a step.

**From requirements:**

Auto-generate a plan from selected requirements:

```
POST /api/plans/generate-from-requirements
```

**In the web UI:**

Visit `/plans/new` for a full plan creation experience with:
- Template selector
- Markdown paste mode
- Bulk step entry
- Requirement linking
- Keyboard shortcuts

### Working with Steps

Each step has a status: `pending`, `in_progress`, `done`, `skipped`, or `blocked`.

```bash
# Mark a step as done
fubbik plan step-done <plan-id> <step-number>

# Add a new step
fubbik plan add-step <plan-id> "Deploy to staging"
```

Steps can:
- Be nested (sub-steps under parent steps)
- Link to requirements for traceability
- Link to chunks for context
- Have notes attached

### Plan Lifecycle

1. **Draft** — initial creation, editing steps
2. **Active** — work in progress
3. **Completed** — all steps done (or auto-completed via session)
4. **Archived** — no longer relevant

```bash
fubbik plan activate <id>
fubbik plan complete <id>
```

## Implementation Sessions

Sessions track a focused work period — typically an AI agent implementing a plan.

### Starting a Session

```bash
POST /api/sessions
{
    "title": "Implement auth flow",
    "planId": "<plan-id>"
}
```

Via MCP: `begin_implementation` tool.

### During a Session

Track what happens:

- **Chunk references** — which knowledge chunks were consulted
- **Assumptions** — knowledge gaps identified during work
- **Requirement references** — which requirements are being addressed, with step-level tracking

### Completing a Session

When a session completes:

1. The linked plan is auto-completed (if all steps are done)
2. Requirement statuses are auto-synced (marking them as passing)
3. A **review brief** is generated summarizing:
   - What was implemented
   - Which requirements were addressed
   - What assumptions were made
   - Coverage analysis

### Review Workflow

Visit `/reviews` to see:

- **Implementation sessions** — completed sessions with review briefs
- **Review queue** — AI-drafted chunks waiting for human approval

Sessions can be marked as reviewed with per-requirement status updates.

## Traceability

The full traceability chain:

```
Requirement → Plan Step → Implementation Session → Chunk
```

View this at `/coverage` to verify:
- Every requirement has a plan step addressing it
- Every plan step has been implemented
- Every implementation created or modified relevant chunks
- The knowledge base reflects the actual implementation
