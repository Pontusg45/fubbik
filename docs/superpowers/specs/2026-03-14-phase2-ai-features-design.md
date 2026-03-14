# Phase 2: AI-Powered Features — Design Outline

## Overview

These features build on the foundation from Phase 1 (multi-codebase, richer chunks, requirements system). They are listed here as planned work — each needs its own detailed spec before implementation.

## Features

### 1. Chunk Export as System Prompt

Export selected chunks as a formatted system prompt optimized for LLM consumption.

- Export by codebase, tags, or manual selection
- Formats: plain markdown, XML-tagged sections, structured JSON
- Deduplication: avoid repeating information across chunks
- Token estimation: show approximate token count for the export
- API: `GET /api/chunks/export/prompt?codebaseId=&tags=&format=`
- CLI: `fubbik context --codebase myproject --format markdown | pbcopy`

### 2. CLAUDE.md / AGENTS.md Generator

Auto-generate project instruction files from codebase chunks.

- Scans chunks for a codebase, filters by type (convention, architecture, runbook)
- Generates a structured CLAUDE.md with sections: Project Overview, Tech Stack, Architecture, Conventions, Commands
- Uses chunk metadata (appliesTo, fileReferences) to organize content by relevance
- API: `GET /api/codebases/:id/generate-instructions?format=claude|agents`
- CLI: `fubbik generate claude.md --codebase myproject > CLAUDE.md`
- Web UI: button on codebase detail page

### 3. Onboarding Mode

"New to this codebase? Here are the most important things to know."

- Ranks chunks by: connection count (hub score), type priority (architecture > convention > note), recency
- Shows top 10-15 chunks in a guided reading order
- Web UI: `/onboarding?codebase=<id>` — a focused, linear reading experience
- Could also power a "Getting Started" export

### 4. "Ask Fubbik" (RAG Q&A)

Natural language Q&A that synthesizes answers from multiple chunks.

- Uses existing embedding infrastructure (Ollama nomic-embed-text)
- Query flow: embed question → semantic search for top-K chunks → LLM synthesis with context
- Supports follow-up questions (conversation context)
- CLI: `fubbik ask "what's our auth strategy?"`
- Web UI: chat-like interface or search box with synthesized answers
- API: `POST /api/ask { question, codebaseId? }`
- Depends on: Ollama running with both embedding + generation models

## Dependencies

All Phase 2 features depend on:
- Multi-codebase support (done)
- Richer chunks with appliesTo + fileReferences (done)
- Templates (done)
- Requirements system (done)

"Ask Fubbik" additionally depends on:
- Ollama with llama3.2 (generation model)
- Existing semantic search infrastructure

## Recommended Order

1. **Chunk Export as System Prompt** — smallest scope, most immediately useful
2. **CLAUDE.md Generator** — builds on export, adds structure
3. **Onboarding Mode** — ranking algorithm + focused UI
4. **Ask Fubbik** — biggest effort, needs LLM integration

Each feature gets its own spec → plan → implementation cycle.
