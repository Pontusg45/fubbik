---
tags:
  - guide
  - chunks
description: How to create, organize, and maintain chunks
---

# Working with Chunks

Chunks are the building blocks of your knowledge base. Each one represents a discrete unit of knowledge — small enough to be useful on its own, rich enough to capture context.

## Creating Chunks

Navigate to **Chunks > New** or press `n` on any page. Fill in:

- **Title** — a clear, descriptive name
- **Content** — markdown-formatted knowledge
- **Type** — note, document, reference, schema, or checklist
- **Tags** — categorize with tags (e.g., "backend", "auth")

**Templates** pre-fill content structure. Choose from built-in templates (Convention, Architecture Decision, Runbook, API Endpoint) or create your own at `/templates`.

**Duplicate detection** warns you if a chunk with similar content already exists (requires Ollama).

## Chunk Types

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

## File References

Link chunks to specific files in your codebase:

- **Applies To** — glob patterns like `src/auth/**` that define which code areas a chunk is relevant to
- **File References** — specific files with optional symbol anchors like `src/auth/session.ts#SessionManager`

These enable AI tools (MCP, VS Code extension) to know which conventions and knowledge apply to which code.

## Tags and Organization

Tags are the primary way to categorize chunks. Tags can have **tag types** (categories with colors) for visual grouping — e.g., a "domain" tag type with tags like "auth", "payments", "notifications".

Use tags for:
- Domain areas (auth, billing, infra)
- Maturity (draft, reviewed, approved)
- Audiences (frontend, backend, devops)

## Health Scores

Each chunk has a health score (0-100) computed from:

- **Freshness** (0-25) — days since last update
- **Completeness** (0-25) — has rationale, alternatives, consequences
- **Richness** (0-25) — content length + AI enrichment (summary, aliases)
- **Connectivity** (0-25) — number of connections to other chunks

Visit `/knowledge-health` to find orphan chunks (no connections), stale chunks (outdated), and thin chunks (too short).

## AI Enrichment

With Ollama running locally, fubbik can auto-generate:

- **Summary** — a one-line description
- **Aliases** — alternative names for search
- **Not About** — terms this chunk is NOT about (improves search precision)
- **Embeddings** — vector representations for semantic search

Enrichment runs automatically when you create or edit a chunk. You can also bulk-enrich via the CLI: `fubbik enrich --all`.
