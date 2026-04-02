---
tags:
  - guide
  - connections
description: How to create and use connections between chunks
---

# Connections

Connections are directed edges between chunks that form your knowledge graph. They capture how ideas relate to each other.

## Connection Types

| Relation | Meaning | Example |
|----------|---------|---------|
| `related_to` | General relationship | "Auth Flow" ↔ "Session Management" |
| `part_of` | Composition/containment | "Login Step" → part_of → "Auth Flow" |
| `depends_on` | Functional dependency | "API Routes" → depends_on → "Database Schema" |
| `extends` | Builds upon | "OAuth Support" → extends → "Auth Flow" |
| `references` | Cites or links to | "API Docs" → references → "Error Codes" |
| `supports` | Provides evidence for | "Load Test Results" → supports → "Scaling Decision" |
| `contradicts` | Conflicts with | "Monolith Approach" → contradicts → "Microservices Decision" |
| `alternative_to` | Different option | "Redis Sessions" → alternative_to → "JWT Sessions" |

## Creating Connections

### In the Web UI

1. Open a chunk's detail page
2. Scroll to the "Connections" section
3. Click "Add Connection"
4. Search for the target chunk
5. Select the relation type
6. Choose the direction (outgoing or incoming)

### Via the CLI

```bash
# Link two chunks
fubbik link <source-id> <target-id> --relation depends_on

# Remove a connection
fubbik unlink <source-id> <target-id>
```

### Via the Graph

Right-click a node in the graph view to access connection options. You can also Alt+Click two nodes to find existing paths between them.

## Viewing Connections

Connections appear in several places:

- **Chunk detail page** — lists incoming and outgoing connections with relation badges
- **Graph view** — visual edges between nodes, colored by relation type
- **Path finding** — discover how two chunks are connected through intermediate nodes
- **Context export** — connections influence which chunks are included in token-budgeted exports

## Best Practices

**Use specific relation types.** "related_to" is a catch-all — prefer more specific types when they apply. "depends_on" tells you about build order; "part_of" tells you about structure; "contradicts" flags conflicts.

**Keep connections directional.** The direction matters: "A depends_on B" means A needs B, not the other way around. Think about which chunk is the source and which is the target.

**Connect across codebases.** Connections are global — they work across codebase boundaries. Use them to link frontend conventions to backend APIs, or shared libraries to their consumers.

**Prune dead connections.** When you delete or significantly change a chunk, review its connections. Stale connections add noise to the graph and context exports.
