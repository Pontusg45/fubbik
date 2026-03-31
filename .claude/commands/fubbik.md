# Knowledge-Integrated Development with Fubbik

You have access to the `fubbik` CLI — a local-first knowledge framework. Use it to pull context before working, capture decisions as you go, and track progress against plans.

## Before You Start

Check if the server is running and detect the current codebase:

```bash
fubbik health
fubbik codebase current
```

If health fails, the server isn't running. You can still use local commands (`fubbik list`, `fubbik add`, `fubbik search`) but server-dependent features (semantic search, enrichment, plans) won't work.

## Workflow Guide

Use this decision tree to determine which commands to run. You don't need to follow every step — pick what's relevant to your current task.

### Starting a task

Before writing code, check what the knowledge base knows about the area you're working in:

```bash
# Get chunks relevant to a specific file (conventions, decisions, references)
fubbik context-for <file-path> --json

# Get conventions for the current codebase
fubbik list --tag convention --json

# If working on a plan, check current progress
fubbik plan list --json
fubbik plan show <plan-id> --json
```

**When to do this:** Always before starting implementation. The context often contains conventions, gotchas, and architectural decisions that affect your approach.

### Searching for knowledge

When you need to find something specific:

```bash
# Text search across all chunks
fubbik search "<query>" --json

# Semantic search (requires Ollama + server)
fubbik search "<query>" --semantic --json

# List chunks filtered by type or tag
fubbik list --type convention --json
fubbik list --tag <tag-name> --json
fubbik list --type reference --json

# Get full details of a specific chunk
fubbik get <chunk-id> --json
```

### Capturing knowledge

When you make a decision, discover a pattern, or learn something worth preserving:

**Architecture decision:**
```bash
fubbik add --template "Architecture Decision" --title "Use X for Y" --content "## Context\n...\n## Decision\n...\n## Consequences\n..." --tag architecture
```

**Convention or pattern:**
```bash
fubbik add --type convention --title "Always do X when Y" --content "..." --tag convention
```

**Quick note:**
```bash
fubbik add --type note --title "Note about X" --content "..."
```

**Reference documentation:**
```bash
fubbik add --type reference --title "How X works" --content "..."
```

**When to capture:** After making a non-obvious decision, discovering a pattern that others should follow, or finding something that took effort to figure out. Don't capture trivial things — the knowledge base should contain what's genuinely useful.

### Connecting knowledge

When chunks are related to each other:

```bash
# Create a connection between two chunks
fubbik link <source-id> <target-id> --relation <type>
```

Relation types: `related_to`, `part_of`, `depends_on`, `extends`, `references`, `supports`, `contradicts`, `alternative_to`

### Tracking plan progress

When working through a plan:

```bash
# List active plans
fubbik plan list --json

# See plan details and steps
fubbik plan show <plan-id> --json

# Mark a step as done
fubbik plan step-done <plan-id> <step-id>

# Add an unplanned step
fubbik plan add-step <plan-id> --description "..."
```

### Checking file coverage

When you want to know if files have associated knowledge:

```bash
# Check specific files against the knowledge base
fubbik check-files <file1> <file2> ...

# Check staged files (useful before commit)
fubbik check-files --staged
```

### Maintenance

```bash
# Generate CLAUDE.md from tagged chunks
fubbik sync-claude-md

# Export context for a directory
fubbik context-dir <dir>

# Enrich chunks with AI-generated summaries (requires Ollama)
fubbik enrich --all
```

## Output Modes

- **Human-readable** (default): Use when displaying results to the user
- **JSON** (`--json`): Use when you need to parse the output programmatically
- **Quiet** (`-q`): Returns only IDs — useful for piping between commands

## Codebase Scoping

Most commands auto-detect the codebase from the git remote. You can override:

```bash
# Scope to a specific codebase
fubbik list --codebase <name> --json

# Show global chunks (not tied to any codebase)
fubbik list --global --json
```

## Common Patterns

**Before implementing a feature:**
```bash
fubbik context-for src/features/auth/login.tsx --json
fubbik list --tag convention --codebase fubbik --json
```

**After making an architecture decision:**
```bash
fubbik add --type document --title "Use Effect for error handling" \
  --content "We use the Effect library for typed errors in the service layer..." \
  --tag architecture --tag error-handling
```

**Recording a convention you discovered:**
```bash
fubbik add --type convention --title "Always use render prop, not asChild" \
  --content "base-ui components use the render prop pattern, NOT Radix asChild..." \
  --tag ui --tag convention
```

**Updating plan progress after completing work:**
```bash
fubbik plan show <plan-id> --json   # find the step ID
fubbik plan step-done <plan-id> <step-id>
```
