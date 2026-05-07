---
tags:
  - guide
  - health
  - maintenance
description: Keeping your knowledge base healthy and up-to-date
---

# Health and Maintenance

A knowledge base is only useful if it's accurate and current. Fubbik provides tools to identify and fix knowledge that needs attention.

## Knowledge Health Dashboard

Visit `/knowledge-health` for a comprehensive overview:

- **Orphan Chunks** — no connections to other chunks. Connect them or archive.
- **Stale Chunks** — not updated recently while connected chunks have changed. Review and update.
- **Thin Chunks** — very little content. Expand, merge, or verify brevity is intentional.
- **Stale Embeddings** — content changed since last embedding. Run `fubbik enrich --all`.
- **File Reference Issues** — file paths that may no longer exist. Update the references.

## Staleness Detection

Proactive detection of chunks that may need attention:
- **Age-based**: flags chunks not updated in 90+ days (configurable)
- **File-changed**: flags chunks linked to files that have been modified
- **Diverged duplicates**: flags chunk pairs with similar but diverging content

Flags are dismissable or permanently suppressible. Surfaced as amber banners on chunk detail pages and in the dashboard "Attention Needed" widget.

## Maintenance Workflow

1. **Weekly:** Check `/knowledge-health` for new orphans and stale chunks
2. **After major changes:** Run `fubbik docs sync` to re-import updated documentation
3. **Monthly:** Review thin chunks and low health scores
4. **Quarterly:** Audit connections for relevance, archive outdated chunks

## CLI Diagnostics

```bash
fubbik health          # System health check
fubbik stats           # Aggregate statistics
```
