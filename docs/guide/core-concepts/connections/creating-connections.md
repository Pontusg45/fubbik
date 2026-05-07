---
tags:
  - guide
  - connections
description: How to create connections via web UI, CLI, and graph
---

# Creating Connections

## In the Web UI

1. Open a chunk's detail page
2. Scroll to the "Connections" section
3. Click "Add Connection"
4. Search for the target chunk
5. Select the relation type
6. Choose the direction (outgoing or incoming)

## Via the CLI

```bash
# Link two chunks
fubbik link <source-id> <target-id> --relation depends_on

# Remove a connection
fubbik unlink <source-id> <target-id>
```

## Via the Graph

Right-click a node in the graph view to access connection options. You can also Alt+Click two nodes to find existing paths between them.

## Viewing Connections

Connections appear in several places:

- **Chunk detail page** — lists incoming and outgoing connections with relation badges
- **Graph view** — visual edges between nodes, colored by relation type
- **Path finding** — discover how two chunks are connected through intermediate nodes
- **Context export** — connections influence which chunks are included in token-budgeted exports
