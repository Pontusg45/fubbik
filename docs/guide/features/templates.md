---
tags:
  - guide
  - templates
description: Built-in and custom chunk templates
---

# Templates

Templates provide pre-filled content structures for common chunk types, helping you create consistent documentation.

## Built-in Templates

### Convention
For coding standards and team agreements: convention description, rationale, examples, and exceptions.

### Architecture Decision
For documenting significant technical choices (ADR format): context, decision, rationale, alternatives considered, and consequences.

### Runbook
For operational procedures: trigger, prerequisites, numbered steps, verification, and rollback.

### API Endpoint
For API documentation: endpoint, description, parameters table, response format, and error codes.

## Using Templates

### In the Web UI

1. Go to `/chunks/new`
2. Click "Use Template"
3. Select a template — content is pre-filled
4. Fill in the sections and save

### Via the CLI

```bash
fubbik add "My Convention" --template "Convention"
```

## Custom Templates

Create your own templates at `/templates`:

1. Click "New Template"
2. Give it a name and description
3. Write the template content (markdown with section headers)
4. Save

Custom templates appear alongside built-in ones. You can also duplicate a built-in template and customize it. Built-in templates are read-only and cannot be modified or deleted.
