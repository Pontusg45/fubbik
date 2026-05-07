---
tags:
  - guide
  - review
  - archive
description: Managing archived chunks — restore and permanent delete
---

# Archived Chunks

Archived chunks are hidden from normal views but preserved for history. Access them at `/chunks/archived`.

## Archiving

Archive a chunk instead of deleting it to:
- Preserve connections and version history
- Keep it findable if you need it later
- Avoid breaking references from other chunks

```bash
fubbik update <id> --archive
```

Or use the "Archive" button on the chunk detail page.

## Restoring

At `/chunks/archived`, click "Restore" to bring a chunk back to active status. All its connections and metadata are preserved.

## Permanent Deletion

If you're sure you no longer need an archived chunk, use "Delete permanently" from the archived view. This removes the chunk, its connections, its version history, and all associated file references.
