---
tags:
  - guide
  - chunks
  - health
description: How chunk health scores are computed and used
---

# Health Scores

Each chunk has a health score (0-100) computed from four dimensions:

| Dimension | Points | What It Measures |
|-----------|--------|-----------------|
| Freshness | 0-25 | Days since last update |
| Completeness | 0-25 | Has rationale, alternatives, consequences |
| Richness | 0-25 | Content length + AI enrichment (summary, aliases) |
| Connectivity | 0-25 | Number of connections to other chunks |

## Where Scores Appear

- **Chunk detail page** — badge next to the title
- **Knowledge health dashboard** (`/knowledge-health`) — sortable overview
- **Context export** — higher-scoring chunks are prioritized in token-budgeted exports

## Improving Scores

- **Freshness**: edit the chunk (even minor tweaks reset the timer)
- **Completeness**: add rationale, alternatives, and consequences
- **Richness**: expand content, run AI enrichment
- **Connectivity**: create connections to related chunks
