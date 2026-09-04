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
fubbik context export --max-tokens 4000

# Boost relevance for a specific file
fubbik context export --for src/auth/session.ts

# Generate focused context for a file
fubbik context for src/auth/session.ts --max-tokens 8000
```

## CLAUDE.md Sync

Generate and maintain a `.claude/CLAUDE.md` file from tagged chunks:

```bash
# One-time generation
fubbik context claude-md --output .claude/CLAUDE.md

# Regenerate the path configured by claude-md.output
fubbik sync

# Preview without writing
fubbik sync --dry-run
```

## Health and Diagnostics

```bash
fubbik health          # System health check
fubbik doctor          # Configuration and connectivity report
```

## Shell Completions

```bash
fubbik completions zsh
fubbik completions bash
fubbik completions fish
```

## Project Configuration

```bash
fubbik init --server http://localhost:3100
fubbik config show
fubbik config set context.max-tokens 8000
fubbik config set claude-md.output .claude/CLAUDE.md
```

Command-line flags override `FUBBIK_URL`, which overrides the nearest
`fubbik.config.json` found while walking up from the current directory.
