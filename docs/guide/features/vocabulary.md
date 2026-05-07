---
tags:
  - guide
  - vocabulary
description: Controlled vocabulary management for BDD requirements
---

# Vocabulary

The vocabulary system at `/vocabulary` provides a controlled dictionary of valid words for writing BDD requirements. It ensures consistency in how requirements are worded across your team.

## Word Categories

| Category | Purpose | Examples |
|----------|---------|---------|
| `actor` | Who performs the action | user, admin, system, moderator |
| `action` | What is done | clicks, creates, deletes, searches |
| `target` | What is acted upon | chunk, codebase, tag, plan |
| `outcome` | What results | sees, receives, is redirected |
| `state` | Preconditions | logged in, on the dashboard, authenticated |
| `modifier` | Qualifiers | successfully, immediately, optionally |

## Real-Time Validation

The requirement step builder at `/requirements/new` validates your text in real-time against the vocabulary:
- **Known words** — highlighted in green
- **Unknown words** — highlighted in amber with suggestions

## AI-Powered Suggestions

Click "Suggest from Chunks" to let the AI analyze your existing chunks and suggest relevant vocabulary words you might be missing.

## Bulk Operations

Import multiple words at once by pasting a list or uploading a file. Each line should be in the format: `word, category`.
