---
tags:
  - guide
  - graph
  - layouts
description: Graph layout algorithms — force-directed, hierarchical, and radial
---

# Graph Layouts

Three layout algorithms are available:

## Force-Directed (Default)

Nodes repel each other, connections attract. Natural clustering emerges. Tag grouping clusters related chunks visually — enable a tag type to see chunks grouped by their tags.

## Hierarchical

Top-down layered layout showing dependency chains. Best for viewing `depends_on` and `part_of` relationships as a tree structure.

## Radial

Spoke pattern radiating from the most-connected node. Useful for seeing which chunks are central to your knowledge base.

## Saved Graph Views

Save your current graph configuration (layout, filters, positions) as a named view that you can return to later. Useful for recurring review sessions or onboarding walkthroughs.
