# AI Requirement Suggestions

## Overview

Enable a workflow where the AI agent suggests improvements by analyzing the knowledge base, then batch-creates approved requirements organized into use cases. The AI agent (Claude Code) generates the suggestions using its own intelligence; Fubbik provides the context and handles creation.

## Workflow

1. AI calls `suggest_requirements` MCP tool with optional focus area → gets structured context
2. AI analyzes context (existing requirements, coverage gaps, health issues, relevant chunks) and generates requirement suggestions in conversation
3. Developer reviews, approves, edits, or rejects suggestions in conversation
4. AI calls `create_requirements_batch` MCP tool to create approved requirements with use case assignments

## 1. MCP Tool: `suggest_requirements`

Input:
```typescript
{
  focus?: string,   // e.g., "auth", "api error handling", "testing"
  codebaseId?: string
}
```

Calls `GET /api/requirements/suggest-context?focus=<focus>&codebaseId=<id>`.

Returns structured context formatted for AI consumption:
- Existing use cases with their requirements (titles + statuses)
- Coverage gaps: uncovered chunks matching the focus
- Knowledge health issues matching the focus
- Relevant chunks matching the focus (title + truncated content, max 20)

The AI uses this context to generate requirement suggestions. No Ollama involved — the agent's own intelligence handles generation.

## 2. MCP Tool: `create_requirements_batch`

Input:
```typescript
{
  requirements: Array<{
    title: string;
    description?: string;
    steps: Array<{ keyword: "given"|"when"|"then"|"and"|"but"; text: string }>;
    priority?: "must"|"should"|"could"|"wont";
    useCaseId?: string;          // existing use case ID
    useCaseName?: string;        // look up or create by name
    parentUseCaseName?: string;  // look up or create parent, nest useCaseName under it
  }>;
  codebaseId?: string;
}
```

Calls `POST /api/requirements/batch`.

The API resolves use cases by name (creating them if needed), validates all steps, and creates all requirements in a single transaction. Returns created count, requirement IDs, and any use cases that were auto-created.

## 3. API Endpoints

### `GET /api/requirements/suggest-context`

Query: `{ focus?: string, codebaseId?: string }`

Response:
```typescript
{
  useCases: Array<{
    id: string;
    name: string;
    parentId: string | null;
    requirementCount: number;
    requirements: Array<{ id: string; title: string; status: string }>;
  }>;
  coverageGaps: Array<{ id: string; title: string; type: string }>;
  healthIssues: {
    orphanCount: number;
    staleCount: number;
    thinCount: number;
  };
  relevantChunks: Array<{
    id: string;
    title: string;
    content: string; // truncated to 300 chars
  }>;
}
```

**When `focus` is provided:**
- Chunks filtered by ILIKE on title and content matching focus
- Coverage gaps filtered by ILIKE on chunk title
- Use cases filtered to only those with requirements matching the focus
- Max 20 relevant chunks with truncated content

**When `focus` is omitted:**
- All use cases with requirement summaries
- Top 10 uncovered chunks (title only, no content)
- Health issue counts only
- No relevant chunks section

### Service

`getSuggestContext(userId, query: { focus?, codebaseId? })`:
1. Fetch use cases with their requirements (using existing `listUseCases` + `listRequirementsByUseCase`, or a combined query)
2. Fetch coverage gaps: call existing `getChunkCoverage` and extract uncovered chunks, optionally filtered by focus
3. Fetch health issues: call existing `getOrphanChunks`/`getStaleChunks`/`getThinChunks` for counts
4. If focus provided: fetch matching chunks via `listChunks` with search parameter

### `POST /api/requirements/batch`

Body:
```typescript
{
  requirements: Array<{
    title: string;
    description?: string;
    steps: Array<{ keyword: string; text: string }>;
    priority?: string;
    useCaseId?: string;
    useCaseName?: string;
    parentUseCaseName?: string;
  }>;
  codebaseId?: string;
}
```

Response:
```typescript
{
  created: number;
  requirements: Array<{ id: string; title: string; useCaseId: string | null }>;
  useCasesCreated: Array<{ id: string; name: string; parentId: string | null }>;
}
```

### Service

`batchCreateRequirements(userId, body)`:
1. **Resolve use cases:** For each unique `useCaseName` + `parentUseCaseName` combination:
   - If `useCaseId` provided, use it directly
   - If `parentUseCaseName` provided, look up by name (and userId). Create if not found.
   - If `useCaseName` provided, look up by name under the parent (if any). Create if not found, linking to parent.
   - Cache resolved IDs to avoid duplicate creation within the same batch
2. **Validate steps:** Run `validateSteps` on each requirement. Collect all errors. If any requirement has invalid steps, reject the entire batch with a `StepValidationError` listing all issues.
3. **Create requirements:** Insert all requirements with generated UUIDs, resolved `useCaseId`, and `codebaseId`. Set `origin: "ai"`, `reviewStatus: "draft"`.
4. Return created requirements and any auto-created use cases.

### Route Registration

Both endpoints go in `packages/api/src/requirements/routes.ts`:
- `GET /requirements/suggest-context` — registered before `/:id` routes (alongside stats, bulk, reorder)
- `POST /requirements/batch` — registered before `/:id` routes

## Files to Create/Modify

### New files
- `packages/api/src/requirements/suggest-context-service.ts` — context gathering logic
- `packages/api/src/requirements/batch-service.ts` — batch creation with use case resolution
- `packages/mcp/src/suggestion-tools.ts` — two MCP tool definitions

### Modified files
- `packages/api/src/requirements/routes.ts` — add suggest-context and batch endpoints
- `packages/mcp/src/index.ts` — register suggestion tools

## Out of Scope

- Server-side AI generation (Ollama) — the agent generates suggestions
- Proposals/drafts table — the conversation is the review gate
- Web UI for suggestions — this is a CLI/MCP workflow
- Chunk linking during batch creation — can be done after via existing API
