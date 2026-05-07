---
tags:
  - guide
  - chunks
  - files
description: Linking chunks to files via applies-to globs and file references
---

# File References

Link chunks to specific files in your codebase for contextual retrieval by AI tools and the VS Code extension.

## Applies To (Glob Patterns)

Define which code areas a chunk is relevant to using glob patterns:

- `src/auth/**` — all files in the auth directory
- `*.test.ts` — all test files
- `packages/api/src/*/routes.ts` — all route files

These patterns are used by the context pipeline to boost chunk relevance when working on matching files.

## File References (Specific Files)

Link to specific files with optional symbol anchors:

- `src/auth/session.ts` — the whole file
- `src/auth/session.ts#SessionManager` — a specific class or function

## Reverse Lookup

Find which chunks reference a given file:

```
GET /api/file-refs/lookup?path=src/auth/session.ts
```

The VS Code extension uses this to automatically surface relevant chunks when you open a file.
