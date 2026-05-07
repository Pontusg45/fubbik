---
tags:
  - guide
  - connections
  - best-practices
description: Guidelines for effective knowledge linking
---

# Connection Best Practices

**Use specific relation types.** "related_to" is a catch-all — prefer more specific types when they apply. "depends_on" tells you about build order; "part_of" tells you about structure; "contradicts" flags conflicts.

**Keep connections directional.** The direction matters: "A depends_on B" means A needs B, not the other way around. Think about which chunk is the source and which is the target.

**Connect across codebases.** Connections are global — they work across codebase boundaries. Use them to link frontend conventions to backend APIs, or shared libraries to their consumers.

**Prune dead connections.** When you delete or significantly change a chunk, review its connections. Stale connections add noise to the graph and context exports.

**Use part_of for hierarchy.** When one chunk is a section or component of another, use `part_of`. This creates a natural tree structure in the graph.

**Avoid cycles in depends_on.** Circular dependencies make the graph harder to reason about. If A depends_on B and B depends_on A, consider whether one direction is stronger.
