---
tags:
  - guide
  - editing
  - autosave
description: Draft auto-saving for chunk creation and editing
---

# Autosave

Fubbik automatically saves drafts to localStorage while you're creating or editing chunks, protecting against data loss from accidental navigation or browser crashes.

## How It Works

- Drafts are saved every few seconds while you type (debounced)
- When you return to the create/edit page, any existing draft is loaded
- Drafts are cleared when you successfully save the chunk
- A "Draft saved" indicator shows when autosave fires

## Draft Recovery

If you navigate away from an unsaved chunk:
1. The draft is preserved in localStorage
2. When you return to the page, you'll see a "Resume draft?" prompt
3. Click to restore your previous content, or dismiss to start fresh

## Limitations

- Drafts are per-browser (stored in localStorage, not synced)
- Only one draft per page (new chunk creation or editing a specific chunk)
- Drafts don't preserve tag or file reference selections — only text content
