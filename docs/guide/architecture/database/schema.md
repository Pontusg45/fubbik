---
tags:
    - guide
    - architecture
    - database
    - schema
description: Core database tables and relationships
---

# Schema Overview

## Core Tables

- **chunk** — the central entity with title, content, type, scope, embedding
- **chunk_connection** — directed edges between chunks with relation types
- **tag** / **tag_type** — tag management with colored categories
- **chunk_tag** — many-to-many chunk-tag associations
- **space** / **space_code_metadata** — knowledge scope and associated repository metadata
- **chunk_space** — many-to-many chunk-space associations
- **workspace** / **workspace_space** — workspace grouping

## Knowledge Metadata

- **chunk_applies_to** — glob patterns linking chunks to file areas
- **chunk_file_ref** — explicit file/symbol references
- **chunk_version** — append-only version history
- **chunk_staleness** — staleness flags with reason and dismiss state

## Planning and Requirements

- **plan** / **plan_task** / **plan_task_dependency** — implementation plans
- **requirement** / **requirement_chunk** — BDD requirements
- **plan_requirement** — plan-requirement links

## Features (Knowledge Overlays)

- **feature** — named overlay with priority and status
- **chunk_feature_delta** — per-chunk field-level modifications
- **user_active_feature** — which features each user has active

## Schema Management

The Rust server applies SQLx migrations from `crates/fubbik-db/migrations/` on startup in development and deployment. Do not use Drizzle migrations for new live schema changes.
