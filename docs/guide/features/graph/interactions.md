---
tags:
  - guide
  - graph
  - interactions
description: Graph navigation, filtering, and path finding
---

# Graph Interactions and Filtering

## Navigation

- **Click** a node to select it and see its details in the sidebar
- **Alt+Click** two nodes to find the shortest path between them
- **Drag** nodes to reposition them
- **Drag** group backgrounds to move all contained nodes together
- **Scroll** to zoom, **drag** the background to pan
- **Right-click** a node for a context menu with quick actions

## Filtering

Use the controls in the toolbar to filter what's shown:

- Filter by chunk **type** (note, document, reference, etc.)
- Filter by connection **relation** (depends_on, part_of, etc.)
- **Search** to highlight matching nodes
- **Tag grouping** — enable tag types to cluster chunks visually into colored groups
- **Show/hide ungrouped** toggle to focus on grouped nodes

## Path Finding

Use the **Find Path** panel (route icon in the toolbar) to discover how two chunks are connected. Select a source and target chunk, and fubbik will find the shortest path between them, showing each connection and its relation type.

## Workspace View

When viewing a workspace (multiple codebases), the graph shows cross-codebase connections with distinct edge styling. This helps you understand how knowledge flows between projects.
