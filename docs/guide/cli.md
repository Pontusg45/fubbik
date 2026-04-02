---
tags:
  - guide
  - cli
description: CLI commands and automation workflows
---

# CLI and Automation

The fubbik CLI lets you manage your knowledge base from the terminal. It auto-detects your codebase from the git remote.

## Setup

Initialize fubbik in your project:

```bash
fubbik init
```

This creates a `.fubbik/` directory with local configuration and connects to the server.

## Chunk Management

```bash
# Create a chunk
fubbik add "Auth Flow" --content "Users authenticate via..." --type document

# Interactive creation (opens $EDITOR)
fubbik add -i

# Create from template
fubbik add --template "Architecture Decision"

# List chunks
fubbik list
fubbik list --codebase myproject --tags auth,backend

# Search
fubbik search "authentication"

# View a chunk
fubbik get <id>

# Update
fubbik update <id> --title "New Title"

# Delete
fubbik remove <id>
```

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

## Document Import

Import markdown documentation as structured documents:

```bash
# Import a single markdown file
fubbik docs import docs/guide/getting-started.md

# Import an entire directory
fubbik docs import-dir docs/guide/

# List imported documents
fubbik docs list

# Re-sync changed files from disk
fubbik docs sync

# Render a document back to markdown
fubbik docs render <id>
```

## CLAUDE.md Sync

Generate and maintain a `.claude/CLAUDE.md` file from tagged chunks:

```bash
# One-time generation
fubbik sync-claude-md

# Watch mode (regenerates on changes)
fubbik sync-claude-md --watch
```

## Plans and Requirements

```bash
# Create a plan
fubbik plan create "Implement auth" --template feature-dev

# Import a plan from markdown
fubbik plan import plan.md

# Mark a step as done
fubbik plan step-done <plan-id> <step-number>

# Manage requirements
fubbik requirements add "User login" \
  --step "given: a user exists" \
  --step "when: they enter credentials" \
  --step "then: they are logged in"
```

## Git Integration

Install a git pre-commit hook that checks staged files against your knowledge base:

```bash
fubbik hooks install

# Check files manually
fubbik check-files src/auth/session.ts
fubbik check-files --staged
```

## Health and Diagnostics

```bash
fubbik health          # System health check
fubbik stats           # Aggregate statistics
fubbik doctor          # Diagnose common issues
fubbik lint --score    # Quality scoring
```
