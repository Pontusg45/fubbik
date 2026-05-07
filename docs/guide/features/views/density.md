---
tags:
  - guide
  - views
  - density
description: Knowledge density map showing chunk distribution across file paths
---

# Density Map

The density view at `/density` visualizes how knowledge is distributed across your codebase's file structure using a heat map.

## How It Works

File references and applies-to patterns from chunks are aggregated to build a tree of your codebase's directories. Directories with more associated chunks appear "hotter" (more knowledge-dense).

## Reading the Map

- **Hot areas** (many chunks) — well-documented parts of the codebase
- **Cold areas** (few chunks) — potentially under-documented code
- **Click** a directory to see which chunks reference files in it

## Use Cases

- **Coverage assessment** — identify which parts of your codebase have knowledge gaps
- **Prioritize documentation** — focus on cold areas that are actively developed
- **Architecture review** — see if knowledge distribution matches code importance
