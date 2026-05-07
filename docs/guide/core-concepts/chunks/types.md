---
tags:
  - guide
  - chunks
  - types
description: Chunk types and decision context fields
---

# Chunk Types and Decision Context

## Types

| Type | Use For |
|------|---------|
| `note` | General knowledge, conventions, tips |
| `document` | Architecture docs, guides, imported docs |
| `reference` | API docs, specifications, external links |
| `schema` | Data models, type definitions |
| `checklist` | Step-by-step procedures, runbooks |

## Decision Context

Any chunk can include optional decision context fields:

- **Rationale** — why this decision was made
- **Alternatives** — other options that were considered
- **Consequences** — trade-offs and impacts of the decision

This turns chunks into living Architecture Decision Records (ADRs). Use these fields when documenting "why" matters as much as "what."
