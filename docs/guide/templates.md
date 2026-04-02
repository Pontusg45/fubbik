---
tags:
  - guide
  - templates
description: Using and creating chunk templates
---

# Templates

Templates provide pre-filled content structures for common chunk types, helping you create consistent documentation.

## Built-in Templates

Fubbik ships with four built-in templates:

### Convention

For coding standards and team agreements:

```markdown
## Convention
[Describe the convention]

## Rationale
[Why this convention exists]

## Examples
[Show correct usage]

## Exceptions
[When this convention doesn't apply]
```

### Architecture Decision

For documenting significant technical choices (ADR format):

```markdown
## Context
[What situation led to this decision]

## Decision
[What was decided]

## Rationale
[Why this option was chosen]

## Alternatives Considered
[Other options and why they were rejected]

## Consequences
[Trade-offs and impacts]
```

### Runbook

For operational procedures:

```markdown
## Trigger
[When to use this runbook]

## Prerequisites
[What you need before starting]

## Steps
1. [Step one]
2. [Step two]
3. [Step three]

## Verification
[How to confirm the procedure worked]

## Rollback
[How to undo if something goes wrong]
```

### API Endpoint

For API documentation:

```markdown
## Endpoint
`METHOD /path`

## Description
[What this endpoint does]

## Parameters
| Name | Type | Required | Description |
|------|------|----------|-------------|

## Response
[Response format and examples]

## Errors
[Error codes and meanings]
```

## Using Templates

### In the Web UI

1. Go to `/chunks/new`
2. Click "Use Template"
3. Select a template
4. The content field is pre-filled with the template structure
5. Fill in the sections and save

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

Custom templates appear alongside built-in ones when creating chunks. You can also duplicate a built-in template and customize it.

Built-in templates are read-only and cannot be modified or deleted.
