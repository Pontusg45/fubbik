# Fubbik: AI Agent Indexing Best Practices

Instructions for AI agents that index and populate a Fubbik knowledge base.

## Overview

Fubbik is a knowledge base built around **chunks** (atomic knowledge units) and **connections** (typed relationships between chunks). Your
job is to break down source material into well-structured chunks, assign appropriate types and tags, create meaningful connections, and then
enrich them with AI-generated metadata.

## Data Model

### Chunk

A chunk is an atomic unit of knowledge. Keep each chunk focused on **one concept, one topic, or one procedure**.

| Field     | Type                           | Description                               |
| --------- | ------------------------------ | ----------------------------------------- |
| `title`   | string (max 200)               | Short, descriptive, unique title          |
| `content` | string (max 50000)             | The knowledge itself — markdown supported |
| `type`    | string (max 20)                | Category (see types below)                |
| `tags`    | string[] (max 20, each max 50) | Searchable labels for filtering           |

### Chunk Types

Use these types consistently:

| Type        | When to use                                         |
| ----------- | --------------------------------------------------- |
| `note`      | General knowledge, observations, ideas, memos       |
| `document`  | Structured long-form content, specs, reports        |
| `guide`     | How-to instructions, tutorials, walkthroughs        |
| `reference` | API docs, glossaries, lookup tables, specifications |
| `schema`    | Data models, type definitions, database schemas     |
| `checklist` | Step-by-step procedures, review lists, runbooks     |

### Connection

A connection is a directed, typed edge between two chunks.

| Field      | Type   | Description                                 |
| ---------- | ------ | ------------------------------------------- |
| `sourceId` | string | The chunk this relationship originates from |
| `targetId` | string | The chunk this relationship points to       |
| `relation` | string | Relationship type (see below)               |

### Relation Types

| Relation         | Meaning                            | Example                                    |
| ---------------- | ---------------------------------- | ------------------------------------------ |
| `related_to`     | General association                | "Auth module" ↔ "Session management"       |
| `part_of`        | Hierarchical containment           | "Login form" → "Auth module"               |
| `depends_on`     | Functional dependency              | "API client" → "Auth tokens"               |
| `extends`        | Builds upon / specializes          | "Admin dashboard" → "Base dashboard"       |
| `references`     | Mentions / cites                   | "Architecture doc" → "Database schema"     |
| `supports`       | Provides evidence / backing        | "Benchmark results" → "Performance claims" |
| `contradicts`    | Conflicts with                     | "Old API spec" ↔ "New API spec"            |
| `alternative_to` | Different approach to same problem | "REST API" ↔ "GraphQL API"                 |

## Best Practices

### 1. Chunk Sizing

**Right-sized chunks are critical.** Too large and they become unfocused; too small and context is lost.

- **Target**: 50–300 lines, or roughly one screen of content
- **Split large documents** by heading (H1/H2/H3). Create an index chunk that lists sections, then a sub-chunk per section with `part_of`
  connections back to the index
- **Don't split** if the content is under ~100 lines and covers one coherent topic
- **Each chunk should be independently understandable** — a reader should grasp the chunk without reading others (though connections provide
  context)

### 2. Titles

- Make titles **unique and descriptive** — they serve as the primary identifier in search and the graph
- Use noun phrases, not sentences: "PostgreSQL Connection Pooling" not "How we set up connection pooling"
- Include the domain/scope when ambiguous: "Auth: Session Tokens" not just "Session Tokens"
- Avoid generic titles: "Overview", "Notes", "Misc" — be specific

### 3. Tags

Tags enable filtering and discovery. Apply them systematically:

- **Domain tags**: the broad area (`auth`, `database`, `frontend`, `api`, `deployment`)
- **Technology tags**: specific tech (`postgres`, `react`, `elysia`, `drizzle`)
- **Content tags**: what kind of information (`architecture`, `configuration`, `troubleshooting`, `api-reference`)
- **Lifecycle tags** (when relevant): `deprecated`, `draft`, `stable`, `experimental`

Rules:

- Use lowercase, hyphenated tags: `error-handling` not `ErrorHandling`
- 3–6 tags per chunk is ideal; max 20
- Be consistent — use the same tag across chunks for the same concept
- Check existing tags with `fubbik tags` before inventing new ones

### 4. Content Quality

- Write in **markdown** — use headings, lists, code blocks
- Include **code examples** where they clarify meaning
- Add **context** at the top: what this chunk is about and why it matters
- Keep content **evergreen** — avoid temporal references like "recently" or "last week"
- Don't duplicate content across chunks — instead, create connections

### 5. Connections

Connections make the knowledge base a graph, not a pile of files. They're as important as the chunks themselves.

- **Always create connections** when chunks reference the same concepts, systems, or procedures
- **Use `part_of`** for hierarchical structure (sections of a doc, components of a system)
- **Use `depends_on`** for functional dependencies (this breaks if that changes)
- **Use `references`** for mentions that don't imply dependency
- **Prefer specific relations** over `related_to` — the more precise the relation, the more useful the graph
- **Bidirectional relationships** like `contradicts` and `alternative_to` need only one connection (the graph renders both directions)

### 6. Indexing a Codebase

When indexing a software project:

1. **Start with `fubbik init --scan --dry-run`** to see what the scanner auto-generates
2. **Review and refine** the auto-generated chunks before importing
3. **Add architecture chunks** manually — the scanner finds docs but can't infer architecture
4. **Create a top-level index chunk** (type: `document`) that maps the system's major components
5. **Index by domain, not by file** — group by concept ("Authentication Flow") not by path ("src/auth/login.ts")

Recommended chunk structure for a codebase:

```
Architecture Overview (document)
├── Auth Module (document, part_of)
│   ├── Session Management (reference, part_of)
│   ├── OAuth Flow (guide, part_of)
│   └── Auth Database Schema (schema, part_of)
├── API Layer (document, part_of)
│   ├── REST Endpoints (reference, part_of)
│   └── Error Handling (guide, part_of)
└── Database (document, part_of)
    ├── Schema Overview (schema, part_of)
    └── Migration Guide (guide, part_of)
```

### 7. Indexing Documentation

When converting existing docs (wiki, markdown files, Notion exports):

1. **One topic = one chunk** — don't create a chunk per file if the file covers multiple topics
2. **Preserve structure** with `part_of` connections between parent docs and their sections
3. **Extract checklists** into their own chunks (type: `checklist`)
4. **Extract schemas/data models** into their own chunks (type: `schema`)
5. **Cross-reference** between related guides with `references` connections

### 8. Batch Operations

For large imports, use the bulk API rather than adding one at a time:

```bash
# Generate a JSONL file (one JSON object per line)
cat chunks.jsonl
{"title": "Auth Overview", "content": "...", "type": "document", "tags": ["auth"]}
{"title": "Login Flow", "content": "...", "type": "guide", "tags": ["auth", "login"]}

# Bulk import
fubbik bulk-add --file chunks.jsonl

# Or via the API directly (max 500 chunks per request)
curl -X POST http://localhost:3000/api/chunks/import \
  -H "Content-Type: application/json" \
  -d '{"chunks": [{"title": "...", "content": "...", "type": "note", "tags": ["tag1"]}]}'
```

### 9. Enrichment

After importing chunks, run AI enrichment to generate summaries, aliases, and embeddings:

```bash
# Enrich a single chunk
fubbik enrich <chunk-id>

# Enrich all chunks
fubbik enrich --all
```

Enrichment generates:

- **Summary**: A concise description used in search results and the graph
- **Aliases**: Alternative names the chunk might be known by (improves search)
- **Not-about**: Terms that sound related but aren't (reduces false positives in semantic search)
- **Embedding**: A 768-dim vector for semantic similarity search

**Always enrich after bulk imports.** Enrichment requires Ollama running locally with `nomic-embed-text` and `llama3.2` models.

### 10. Verification

After indexing, verify the quality of your knowledge base:

```bash
# Check stats
fubbik stats

# Search for key terms to verify coverage
fubbik search "authentication"
fubbik search --semantic "how to deploy"

# List chunks by type
fubbik list --type guide
fubbik list --type schema

# Export for review
fubbik export > knowledge-base.json
```

In the web UI:

- Open the **graph view** to visually inspect connections and clustering
- Check for **isolated nodes** (chunks with no connections) — they likely need linking
- Look for **dense clusters** that should be broken into sub-topics
- Use the **metrics panel** to see connection density and hub nodes

## CLI Quick Reference

| Command                                                        | Purpose                                |
| -------------------------------------------------------------- | -------------------------------------- |
| `fubbik init --scan`                                           | Scan project and create initial chunks |
| `fubbik add -t "Title" -c "Content" --type guide --tags "a,b"` | Add a single chunk                     |
| `fubbik bulk-add --file chunks.jsonl`                          | Bulk import from JSONL                 |
| `fubbik import --file docs/`                                   | Import from markdown directory         |
| `fubbik link <source-id> <target-id> -r part_of`               | Create a connection                    |
| `fubbik unlink <source-id> <target-id>`                        | Remove a connection                    |
| `fubbik update <id> --tags "new,tags"`                         | Update chunk metadata                  |
| `fubbik enrich --all`                                          | Run AI enrichment on all chunks        |
| `fubbik sync --url http://localhost:3000`                      | Sync local store with server           |
| `fubbik search "query"`                                        | Text search                            |
| `fubbik search --semantic "query"`                             | AI-powered semantic search             |
| `fubbik list`                                                  | List all chunks                        |
| `fubbik stats`                                                 | Knowledge base statistics              |
| `fubbik export`                                                | Export as JSON                         |

## API Quick Reference

| Endpoint                      | Method | Purpose                                                                    |
| ----------------------------- | ------ | -------------------------------------------------------------------------- |
| `/api/chunks`                 | GET    | List chunks (supports `type`, `tags`, `search`, `sort`, `limit`, `offset`) |
| `/api/chunks`                 | POST   | Create a chunk                                                             |
| `/api/chunks/:id`             | PATCH  | Update a chunk                                                             |
| `/api/chunks/:id`             | DELETE | Delete a chunk                                                             |
| `/api/chunks/import`          | POST   | Bulk import (max 500)                                                      |
| `/api/chunks/export`          | GET    | Export all chunks                                                          |
| `/api/chunks/search/semantic` | GET    | Semantic search (`q`, `limit`)                                             |
| `/api/chunks/bulk`            | DELETE | Bulk delete (`ids[]`, max 100)                                             |
| `/api/connections`            | POST   | Create connection (`sourceId`, `targetId`, `relation`)                     |

## Anti-Patterns

| Don't                                      | Do instead                             |
| ------------------------------------------ | -------------------------------------- |
| Create one giant chunk per document        | Split by heading into focused chunks   |
| Use vague types like "note" for everything | Pick the most specific type            |
| Skip connections                           | Always connect related chunks          |
| Duplicate content across chunks            | Create one chunk and connect it        |
| Use inconsistent tags                      | Check `fubbik tags` first              |
| Import without enriching                   | Always run `fubbik enrich --all` after |
| Create chunks without content              | Every chunk needs meaningful content   |
| Use file paths as titles                   | Use descriptive, human-readable titles |
