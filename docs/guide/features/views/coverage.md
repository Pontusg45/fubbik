---
tags:
  - guide
  - views
  - coverage
  - requirements
description: Requirement coverage and traceability matrices
---

# Coverage

The coverage view at `/coverage` provides two matrices for tracking requirement implementation.

## Chunk Coverage

Shows which chunks satisfy which requirements. The matrix highlights:
- **Covered** — requirements linked to chunks
- **Uncovered** — requirements with no linked chunks (knowledge gaps)
- **Over-covered** — requirements with many chunks (possible redundancy)

## Traceability Matrix

The full chain from requirements through implementation:

```
Requirement → Plan Step → Implementation Session → Chunk
```

This verifies:
- Every requirement has a plan step addressing it
- Every plan step has been implemented
- Every implementation created or modified relevant chunks
- The knowledge base reflects actual implementation

## Knowledge Gap Actions

When the coverage view identifies uncovered requirements, you can click "Create Requirement" to start addressing the gap — linking new or existing chunks to fill the hole.
