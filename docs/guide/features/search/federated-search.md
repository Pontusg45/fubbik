---
tags:
  - guide
  - search
  - federated
description: Cross-codebase search with codebase attribution
---

# Federated Search

Federated search finds chunks across all codebases simultaneously, showing which codebase each result belongs to.

```
GET /api/chunks/search/federated?q=authentication
```

This is useful for finding patterns and conventions that exist across multiple projects — for example, discovering that both the frontend and backend repos have authentication-related chunks with different approaches.

## Use Cases

- **Cross-project audits** — find all chunks about a topic across every codebase
- **Convention alignment** — check if different teams document the same patterns
- **Onboarding** — get a platform-wide view of a concept without switching between codebases
