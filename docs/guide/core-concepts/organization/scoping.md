---
tags:
  - guide
  - codebases
  - scoping
description: How global vs scoped chunks work
---

# Scoping Rules

| Scope | Visible When | Use For |
|-------|-------------|---------|
| Global (no codebase) | Always | Company-wide conventions, shared knowledge |
| Single codebase | That codebase selected | Project-specific decisions, APIs |
| Multiple codebases | Any assigned codebase selected | Shared libraries, cross-project patterns |
| Workspace | Workspace selected | Platform-wide views |

## Scope Metadata

Chunks also have a **scope** field — free-form JSONB key-value pairs for custom metadata. For example:

```json
{ "environment": "production", "team": "platform", "language": "typescript" }
```

## Scope Schema Registry

Optionally register expected scope keys at `/settings` to get autocomplete and validation in the UI. The `scope_key` table defines keys with types (string, number, boolean, enum) and allowed values. Scope remains free-form — the registry is opt-in guidance.
