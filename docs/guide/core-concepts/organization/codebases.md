---
tags:
  - guide
  - codebases
description: Per-project knowledge scoping with codebases
---

# Codebases

A codebase represents a single project or repository. It's identified by its git remote URL (normalized) or local file paths.

## Automatic Detection

The CLI auto-detects your codebase from the current directory's git remote:

```bash
cd ~/projects/my-app
fubbik list  # automatically scoped to my-app's codebase
```

The web UI has a codebase switcher in the navigation bar that lets you filter all views by codebase.

## Managing Codebases

```bash
# Register a new codebase
fubbik codebase add my-app

# List registered codebases
fubbik codebase list

# Show the current codebase
fubbik codebase current

# Remove a codebase
fubbik codebase remove my-app
```

In the web UI, manage codebases at `/codebases`.

## Scoping Chunks to Codebases

Chunks can belong to multiple codebases or none (global). When creating a chunk, you can assign it to specific codebases.

```bash
# Create a chunk for a specific codebase
fubbik add "API Convention" --codebase my-app

# List only global chunks
fubbik list --global

# List chunks across all codebases
fubbik list --all-codebases
```
