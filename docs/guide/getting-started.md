---
tags:
  - guide
  - onboarding
description: Introduction to fubbik and how to get up and running
---

# Getting Started

Welcome to fubbik — a local-first knowledge framework for storing, navigating, and evolving structured knowledge about your codebases.

## What is Fubbik?

Fubbik helps you capture and organize the knowledge that lives in your team's heads: architecture decisions, coding conventions, runbooks, API documentation, and more. It's designed for both humans (web UI, graph visualization) and machines (CLI, MCP server, VS Code extension).

Unlike wikis or docs-as-code tools, fubbik breaks knowledge into **chunks** — small, atomic units that can be connected, tagged, searched, and served to AI tools with token-aware budgeting.

## Core Concepts

**Chunks** are the central unit. Each chunk is a discrete piece of knowledge with a title, markdown content, type, tags, and optional metadata like file references and decision context (rationale, alternatives, consequences).

**Connections** are directed edges between chunks. They have a relation type: `related_to`, `part_of`, `depends_on`, `extends`, `references`, `supports`, `contradicts`, or `alternative_to`. Connections form a knowledge graph you can visualize and navigate.

**Codebases** organize chunks per-project. The CLI auto-detects your codebase from the git remote. Chunks can belong to multiple codebases or none (global).

**Workspaces** group related codebases (e.g., frontend + backend + infra) so you can view knowledge across projects.

## Quick Start

1. Start the dev server: `pnpm dev`
2. Open `http://localhost:3001/dashboard`
3. Create your first chunk — give it a title, some content, and a type
4. Tag it with relevant categories
5. Create more chunks and connect them
6. View the graph at `/graph` to see your knowledge map

## Installation

Clone the repo and install dependencies:

```bash
git clone <repo-url>
cd fubbik
pnpm install
```

Copy the environment file and configure:

```bash
cp .env.example .env
```

Required environment variables:
- `DATABASE_URL` — PostgreSQL connection string
- `BETTER_AUTH_SECRET` — Auth secret (min 32 chars)
- `BETTER_AUTH_URL` — Auth server URL (e.g., `http://localhost:3000`)

Push the database schema and seed sample data:

```bash
pnpm db:push
pnpm seed
```

Start the development server:

```bash
pnpm dev
```

This starts both the API server (port 3000) and web app (port 3001).

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+K` | Open command palette |
| `?` | Show all shortcuts |
| `n` | Create new (context-aware) |
| `e` | Edit current item |
| `Esc` | Go back |
| `j/k` | Navigate lists |
