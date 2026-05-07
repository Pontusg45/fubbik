---
tags:
  - guide
  - cli
  - codebases
description: CLI commands for codebase and workspace management
---

# Codebase and Workspace Commands

## Codebase Management

```bash
# Register a new codebase
fubbik codebase add my-app

# List registered codebases
fubbik codebase list

# Show the current codebase (auto-detected from git remote)
fubbik codebase current

# Remove a codebase
fubbik codebase remove my-app
```

## Scoped Operations

Most CLI commands respect the current codebase context:

```bash
# List chunks for current codebase (auto-detected)
fubbik list

# List global chunks only
fubbik list --global

# List chunks for a specific codebase
fubbik list --codebase my-app

# List across all codebases
fubbik list --all-codebases
```

## Tag Management

```bash
# List tags
fubbik tags list

# Create a tag
fubbik tags add "backend" --type "domain"

# Delete a tag
fubbik tags remove "backend"
```
