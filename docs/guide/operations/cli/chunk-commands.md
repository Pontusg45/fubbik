---
tags:
  - guide
  - cli
  - chunks
description: CLI commands for chunk CRUD and search
---

# Chunk Commands

## Creating Chunks

```bash
# Create a chunk
fubbik add "Auth Flow" --content "Users authenticate via..." --type document

# Interactive creation (opens $EDITOR)
fubbik add -i

# Create from template
fubbik add --template "Architecture Decision"

# Quick one-liner (auto-detects codebase)
fubbik quick "Always use prepared statements" --type note --tags security,backend
```

## Listing and Searching

```bash
# List chunks
fubbik list
fubbik list --codebase myproject --tags auth,backend

# Search by keyword
fubbik search "authentication"

# Semantic search (requires Ollama)
fubbik search "how do we handle user auth" --semantic

# View a chunk
fubbik get <id>
```

## Updating and Deleting

```bash
# Update
fubbik update <id> --title "New Title"

# Delete
fubbik remove <id>
```

## Connections

```bash
# Link two chunks
fubbik link <source-id> <target-id> --relation depends_on

# Remove a connection
fubbik unlink <source-id> <target-id>
```

## Git Integration

```bash
# Install pre-commit hook
fubbik hooks install

# Check files manually
fubbik check-files src/auth/session.ts
fubbik check-files --staged
```
