# Domain context

Fubbik stores durable, structured knowledge for humans and agents. These terms
are the vocabulary used by code, tests, documentation, and command interfaces.

- **Chunk** — the primary unit of knowledge. A Chunk has content, metadata,
  history, tags, Space membership, and connections to other Chunks.
- **Document** — an imported Markdown source whose sections are represented by
  ordered Chunks. Re-importing or syncing reconciles those Chunks.
- **Space** — a scope for related knowledge, commonly associated with a local
  codebase and its remote repository.
- **Plan** — an owned body of work containing ordered tasks, requirements,
  analysis items, and external links.
- **Agent Run** — one durable agent identity participating in a Plan. A child
  run may identify its parent and reconnect through an external key.
- **Claim** — a renewable lease granting an Agent Run temporary ownership of a
  Plan task.
- **Coordination Entry** — an append-only, sequenced message on a Plan board.
  Entries may address a run, refer to a task, or reply to another entry.
- **Built-in Catalog** — required reference data, such as Chunk types and
  connection relations, that must agree across database bootstrap paths.

## Architectural vocabulary

- A **module** hides implementation behind one interface.
- A **seam** is where behavior can vary without editing its caller.
- An **adapter** satisfies an interface at a seam.
- Prefer deep modules: small interfaces with enough implementation to provide
  leverage to callers and locality to maintainers.

