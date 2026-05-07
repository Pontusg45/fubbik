---
tags:
  - guide
  - views
  - browse
  - clusters
  - ai
description: AI-powered topic clustering based on embedding similarity
---

# Topic Clusters

The clusters view at `/browse/clusters` groups similar chunks together using their vector embeddings. This is an AI-powered alternative to manual tag-based organization.

## How It Works

1. Chunks with embeddings are clustered by cosine similarity
2. Each cluster gets a representative label based on the most common tags
3. Similarity scores show how tightly grouped each cluster is

## Requirements

Topic clustering requires Ollama and embeddings to be generated. Chunks without embeddings won't appear in clusters.

## Use Cases

- **Discover hidden relationships** — find chunks that are semantically related but have different tags
- **Identify redundancy** — tight clusters may contain duplicates or overlapping content
- **Audit coverage** — see what topic areas your knowledge base naturally groups into
