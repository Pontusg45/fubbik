---
tags:
  - guide
  - workspaces
description: Grouping related codebases with workspaces
---

# Workspaces

A workspace groups related codebases together. For example, you might have a "Platform" workspace containing your frontend, backend, and infrastructure codebases.

## Creating Workspaces

In the web UI at `/workspaces`:
1. Click "New Workspace"
2. Give it a name and description
3. Add codebases to it

## Cross-Codebase Views

When you select a workspace in the codebase switcher, all views (chunks, graph, search) show data from all codebases in that workspace. This is useful for:

- Seeing how frontend and backend knowledge connects
- Finding conventions that span multiple projects
- Identifying shared patterns across a platform

## Graph Workspace View

The graph view is especially powerful with workspaces. It shows chunks from all member codebases with:
- Different node styles per codebase
- Cross-codebase edges highlighted
- Cluster grouping by codebase
