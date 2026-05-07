---
tags:
  - guide
  - import
  - markdown
description: Importing markdown files as chunks with frontmatter parsing
---

# Markdown Import

Import a single markdown file or an entire directory as chunks:

```bash
# Single file
fubbik import docs/architecture.md --server --codebase my-app

# Directory (recursive)
fubbik import docs/ --server --codebase my-app
```

## Frontmatter

Frontmatter is parsed for metadata:

```markdown
---
title: My Document
type: document
tags:
  - backend
  - architecture
description: Overview of the backend architecture
---

# Content starts here
```

Supported frontmatter fields: `title`, `type`, `tags`, `description`.

## Web UI Import

The `/import` page provides a drag-and-drop interface for importing markdown files with:
- File preview table
- Codebase selection
- Frontmatter extraction preview
- Bulk import with progress tracking

Files in the same directory get automatic `part_of` connections to index files (README.md, index.md).
