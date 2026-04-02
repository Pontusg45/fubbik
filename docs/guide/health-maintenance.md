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

Visit `/knowledge-health` for a comprehensive overview of issues:

### Orphan Chunks

Chunks with no connections to other chunks. They may be:
- New chunks that haven't been linked yet
- Isolated knowledge that should be connected
- Outdated content that can be removed

**Fix:** Review each orphan. Either connect it to related chunks or archive it if it's no longer relevant.

### Stale Chunks

Chunks that haven't been updated in 30+ days but are connected to recently-changed chunks. The connected knowledge has evolved, but these chunks may be out of date.

**Fix:** Review the chunk content against current reality. Update the content or mark it as still accurate by editing (even a minor tweak resets the freshness timer).

### Thin Chunks

Chunks with very little content. They may be:
- Placeholders that were never fleshed out
- Chunks that should be merged into a related chunk
- Valid but minimal entries (like short conventions)

**Fix:** Expand the content, merge into a parent chunk, or verify that the brief content is intentional.

### Stale Embeddings

Chunks whose content has changed since their last embedding was generated. Semantic search results may be inaccurate for these chunks.

**Fix:** Run `fubbik enrich --all` to regenerate embeddings, or enrich individual chunks from their detail page.

### File Reference Issues

File references pointing to files that may no longer exist or have moved.

**Fix:** Update the file paths in the chunk's file references section.

## Health Scores

Each chunk has a health score from 0-100 based on four dimensions:

| Dimension | Points | What It Measures |
|-----------|--------|-----------------|
| Freshness | 0-25 | Days since last update |
| Completeness | 0-25 | Has rationale, alternatives, consequences |
| Richness | 0-25 | Content length + AI enrichment |
| Connectivity | 0-25 | Number of connections |

Health scores appear as badges on chunk detail pages. Use them to prioritize maintenance work.

## CLI Diagnostics

```bash
# System health check
fubbik health

# Detailed diagnostics
fubbik doctor

# Quality scoring
fubbik lint --score

# Knowledge statistics
fubbik stats
```

## Maintenance Workflow

A good maintenance cadence:

1. **Weekly:** Check `/knowledge-health` for new orphans and stale chunks
2. **After major changes:** Run `fubbik docs sync` to re-import updated documentation
3. **Monthly:** Review thin chunks and low health scores
4. **Quarterly:** Audit connections for relevance, archive outdated chunks

## Archiving

Instead of deleting chunks, archive them to preserve history:

- Archived chunks are hidden from normal views but still searchable
- Connections to archived chunks are preserved
- You can restore archived chunks if needed

Archive from the chunk detail page or via the CLI:
```bash
fubbik update <id> --archive
```
