---
tags:
  - guide
  - scope
  - metadata
description: Scope schema registry for chunk metadata validation
---

# Scope Schema Registry

The scope registry is an optional system for defining expected scope keys on chunks. While chunk scope is free-form JSONB, the registry provides autocomplete and validation guidance.

## Registering Scope Keys

At `/settings` or via the API:

```
POST /api/scope-keys
{
  "key": "environment",
  "description": "Deployment environment this chunk applies to",
  "valueType": "enum",
  "allowedValues": ["development", "staging", "production"]
}
```

## Supported Value Types

| Type | Description | Example |
|------|-------------|---------|
| `string` | Free-form text | `"team": "platform"` |
| `number` | Numeric value | `"priority": 1` |
| `boolean` | True/false | `"deprecated": true` |
| `enum` | Constrained values | `"environment": "production"` |

## How It's Used

- **Chunk edit form** — scope key autocomplete suggests registered keys
- **Enum fields** — show a dropdown with allowed values
- **Validation** — optional warning when a scope value doesn't match the registry

The registry is guidance, not enforcement. Chunks can have any scope keys regardless of registry state.
