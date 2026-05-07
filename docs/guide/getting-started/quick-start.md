---
tags:
  - guide
  - onboarding
  - quick-start
description: Your first five minutes with fubbik
---

# Quick Start

1. Start the dev server: `pnpm dev`
2. Open `http://localhost:3001/dashboard`
3. Create your first chunk — give it a title, some content, and a type
4. Tag it with relevant categories
5. Create more chunks and connect them
6. View the graph at `/graph` to see your knowledge map

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+K` | Open command palette |
| `?` | Show all shortcuts |
| `n` | Create new (context-aware) |
| `e` | Edit current item |
| `Esc` | Go back |
| `j/k` | Navigate lists |

## Core Concepts at a Glance

**Chunks** are the central unit. Each chunk is a discrete piece of knowledge with a title, markdown content, type, tags, and optional metadata.

**Connections** are directed edges between chunks. They have a relation type and form a knowledge graph you can visualize and navigate.

**Codebases** organize chunks per-project. The CLI auto-detects your codebase from the git remote.

**Workspaces** group related codebases (e.g., frontend + backend + infra) so you can view knowledge across projects.
