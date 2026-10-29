export interface BuiltinTemplate {
    name: string;
    type: string;
    tags: string[];
    content: string;
}

export const BUILTIN_TEMPLATES: BuiltinTemplate[] = [
    {
        name: "ADR",
        type: "document",
        tags: ["architecture", "decision"],
        content: `## Context

What is the issue that we're seeing that is motivating this decision?

## Decision

What is the change that we're proposing and/or doing?

## Consequences

What becomes easier or more difficult to do because of this change?

## Alternatives Considered

What other options were considered and why were they rejected?`
    },
    {
        name: "Convention",
        type: "convention",
        tags: ["convention"],
        content: `## Rule

State the convention clearly and concisely.

## Why

Why does this convention exist? What problem does it solve?

## Examples

### Do
\`\`\`typescript
// correct usage
\`\`\`

### Don't
\`\`\`typescript
// incorrect usage
\`\`\``
    },
    {
        name: "Runbook",
        type: "document",
        tags: ["runbook", "operations"],
        content: `## When to Use

Describe the situation that triggers this runbook.

## Prerequisites

- [ ] Prerequisite 1
- [ ] Prerequisite 2

## Steps

1. First step
2. Second step
3. Third step

## Verification

How to verify the procedure was successful.

## Rollback

How to undo if something goes wrong.`
    },
    {
        name: "API Endpoint",
        type: "reference",
        tags: ["api", "reference"],
        content: `## Endpoint

\`METHOD /api/path\`

## Description

What this endpoint does.

## Request

### Headers
- \`Authorization: Bearer <token>\`

### Body
\`\`\`json
{
}
\`\`\`

## Response

### Success (200)
\`\`\`json
{
}
\`\`\`

### Errors
- \`400\` — Validation error
- \`401\` — Unauthorized
- \`404\` — Not found`
    },
    {
        name: "Reference",
        type: "reference",
        tags: ["reference"],
        content: `## Overview

Brief description of what this documents.

## Details

Detailed explanation.

## Related

- Link to related chunks or resources`
    }
];

export function getBuiltinTemplate(name: string): BuiltinTemplate | undefined {
    return BUILTIN_TEMPLATES.find(t => t.name.toLowerCase() === name.toLowerCase());
}

export function listBuiltinTemplateNames(): string[] {
    return BUILTIN_TEMPLATES.map(t => t.name);
}
