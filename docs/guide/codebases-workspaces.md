---
tags:
  - guide
  - codebases
  - workspaces
description: Organizing knowledge by codebase and workspace
---

# Codebases and Workspaces

Fubbik organizes knowledge per-project using codebases, and groups related projects using workspaces.

## Codebases

A codebase represents a single project or repository. It's identified by its git remote URL (normalized) or local file paths.

### Automatic Detection

The CLI auto-detects your codebase from the current directory's git remote:

```bash
cd ~/projects/my-app
fubbik list  # automatically scoped to my-app's codebase
```

The web UI has a codebase switcher in the navigation bar that lets you filter all views by codebase.

### Managing Codebases

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

### Scoping Chunks

Chunks can belong to multiple codebases or none (global). When creating a chunk, you can assign it to specific codebases.

```bash
# Create a chunk for a specific codebase
fubbik add "API Convention" --codebase my-app

# List only global chunks
fubbik list --global

# List chunks across all codebases
fubbik list --all-codebases
```

## Workspaces

A workspace groups related codebases together. For example, you might have a "Platform" workspace containing your frontend, backend, and infrastructure codebases.

### Creating Workspaces

In the web UI at `/workspaces`:
1. Click "New Workspace"
2. Give it a name and description
3. Add codebases to it

### Cross-Codebase Views

When you select a workspace in the codebase switcher, all views (chunks, graph, search) show data from all codebases in that workspace. This is useful for:

- Seeing how frontend and backend knowledge connects
- Finding conventions that span multiple projects
- Identifying shared patterns across a platform

### Graph Workspace View

The graph view is especially powerful with workspaces. It shows chunks from all member codebases with:
- Different node styles per codebase
- Cross-codebase edges highlighted
- Cluster grouping by codebase

## Global vs Scoped

| Scope | Visible When | Use For |
|-------|-------------|---------|
| Global (no codebase) | Always | Company-wide conventions, shared knowledge |
| Single codebase | That codebase selected | Project-specific decisions, APIs |
| Multiple codebases | Any assigned codebase selected | Shared libraries, cross-project patterns |
| Workspace | Workspace selected | Platform-wide views |
