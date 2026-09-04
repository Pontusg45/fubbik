---
tags:
    - guide
    - cli
    - chunks
description: CLI commands for chunk CRUD and search
---

# Chunk Commands

## Creating Chunks

```bash
# Create a chunk
fubbik add "Auth Flow" --content "Users authenticate via..." --type document

# Read content from a file or stdin
fubbik add "Auth Flow" --file docs/auth.md --tags auth,backend
git diff | fubbik add "Current changes" --stdin

# Associate a chunk with one or more spaces
fubbik add "Prepared statements" --space api --tags security,backend
```

## Listing and Searching

```bash
# List chunks
fubbik list
fubbik list --type document

# Search by keyword
fubbik search "authentication"

# View a chunk
fubbik get <id>

# Print raw content for piping
fubbik cat <id>
```

## Updating and Deleting

```bash
# Update
fubbik update <id> --title "New Title"
fubbik update <id> --file replacement.md --tags auth,current

# Delete interactively, or explicitly confirm in scripts
fubbik delete <id>
fubbik delete <id> --yes
```

## Machine-readable Output

```bash
fubbik --json get <id>
fubbik --quiet add "Automation note" --stdin
```
