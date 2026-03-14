-- Seed built-in chunk templates
INSERT INTO chunk_template (id, name, description, type, content, is_built_in, user_id, created_at)
VALUES
  (
    'builtin-convention',
    'Convention',
    'Template for documenting coding conventions and standards',
    'note',
    '## Rule

[What is the convention?]

## Rationale

[Why does this convention exist?]

## Examples

[Code examples showing correct usage]

## Exceptions

[When is it OK to break this rule?]',
    true,
    NULL,
    NOW()
  ),
  (
    'builtin-architecture-decision',
    'Architecture Decision',
    'Template for recording architecture decision records (ADRs)',
    'document',
    '## Context

[What is the situation that requires a decision?]

## Decision

[What was decided?]

## Alternatives Considered

[What other options were evaluated?]

## Consequences

[What are the trade-offs and impacts?]',
    true,
    NULL,
    NOW()
  ),
  (
    'builtin-runbook',
    'Runbook',
    'Template for operational runbooks and procedures',
    'document',
    '## When to Use

[What situation triggers this runbook?]

## Steps

1. [Step 1]
2. [Step 2]

## Rollback

[How to undo if something goes wrong]

## Escalation

[Who to contact if this doesn''t resolve the issue]',
    true,
    NULL,
    NOW()
  ),
  (
    'builtin-api-endpoint',
    'API Endpoint',
    'Template for documenting API endpoints',
    'reference',
    '## Endpoint

[METHOD /path]

## Request

[Headers, body, query params]

## Response

[Success and error responses]

## Authentication

[Auth requirements]

## Errors

[Error codes and their meaning]',
    true,
    NULL,
    NOW()
  )
ON CONFLICT (name) WHERE user_id IS NULL
DO UPDATE SET
  content = EXCLUDED.content,
  description = EXCLUDED.description,
  type = EXCLUDED.type;
