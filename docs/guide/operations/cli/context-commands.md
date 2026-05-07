---
tags:
  - guide
  - cli
  - context
description: CLI commands for context export and CLAUDE.md generation
---

# Context and Export Commands

## Context Export

Export knowledge for AI consumption with token budgeting:

```bash
# Export up to 4000 tokens of context
fubbik context --max-tokens 4000

# Boost relevance for a specific file
fubbik context --for src/auth/session.ts

# Generate context for a file (with dependency awareness)
fubbik context-for src/auth/session.ts --include-deps

# Generate CLAUDE.md-style context for a directory
fubbik context-dir src/auth/
```

## CLAUDE.md Sync

Generate and maintain a `.claude/CLAUDE.md` file from tagged chunks:

```bash
# One-time generation
fubbik sync-claude-md

# Watch mode (regenerates on changes)
fubbik sync-claude-md --watch
```

## Health and Diagnostics

```bash
fubbik health          # System health check
fubbik stats           # Aggregate statistics
```

## Shell Completions

```bash
fubbik completions zsh
```
