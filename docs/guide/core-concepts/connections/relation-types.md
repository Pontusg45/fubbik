---
tags:
  - guide
  - connections
  - types
description: The eight connection relation types and when to use each
---

# Relation Types

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

## Choosing the Right Type

Use `related_to` as a catch-all only when no specific type fits. Prefer more specific types — they convey meaning that helps the graph view, context export, and AI tools understand your knowledge structure.

- **Structural** relationships: `part_of`, `extends`
- **Dependency** relationships: `depends_on`, `references`
- **Evaluative** relationships: `supports`, `contradicts`, `alternative_to`
