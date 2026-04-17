/**
 * File-level links for chunks: applies_to globs + file_ref direct paths.
 *
 * Both layers let the "context for a file" endpoint find relevant chunks; the
 * graph density map uses the same data for its folder tree view.
 */

import { chunkAppliesTo } from "../../schema/applies-to";
import { chunkFileRef } from "../../schema/file-ref";
import { uuid } from "../factories";
import type { SeedContext } from "../context";

export async function seed(ctx: SeedContext): Promise<void> {
    const c = ctx.ids.chunks;

    const applies: Array<{ chunkName: string; pattern: string; note?: string }> = [
        { chunkName: "arch", pattern: "**/*", note: "Applies to the whole repo" },
        { chunkName: "schema-chunks", pattern: "packages/db/src/schema/chunk.ts" },
        { chunkName: "schema-connection", pattern: "packages/db/src/schema/chunk.ts" },
        { chunkName: "schema-catalogs", pattern: "packages/db/src/schema/chunk-type.ts" },
        { chunkName: "schema-catalogs", pattern: "packages/db/src/schema/connection-relation.ts" },
        { chunkName: "schema-plan", pattern: "packages/db/src/schema/plan.ts" },

        { chunkName: "repo-pattern", pattern: "packages/api/src/**/service.ts" },
        { chunkName: "repo-pattern", pattern: "packages/db/src/repository/**" },
        { chunkName: "typed-errors", pattern: "packages/api/src/errors.ts" },
        { chunkName: "typed-errors", pattern: "packages/api/src/index.ts" },
        { chunkName: "eden-treaty", pattern: "packages/api/src/index.ts" },
        { chunkName: "eden-treaty", pattern: "apps/web/src/utils/api.ts" },
        { chunkName: "env-validation", pattern: "packages/env/**" },

        { chunkName: "catalog-pattern", pattern: "packages/api/src/vocabularies/**" },
        { chunkName: "catalog-pattern", pattern: "apps/web/src/features/vocabularies/**" },
        { chunkName: "scope-jsonb", pattern: "packages/db/src/schema/chunk.ts" },

        { chunkName: "tanstack-start", pattern: "apps/web/src/routes/**" },
        { chunkName: "feature-structure", pattern: "apps/web/src/features/**" },
        { chunkName: "base-ui-render", pattern: "apps/web/src/components/ui/**" },
        { chunkName: "react-flow-graph", pattern: "apps/web/src/features/graph/**" },
        { chunkName: "graph-layout", pattern: "apps/web/src/features/graph/force-layout.ts" },
        { chunkName: "graph-filter-dialog", pattern: "apps/web/src/features/graph/graph-filter-*.tsx" },
        { chunkName: "graph-perf", pattern: "apps/web/src/features/graph/**" },
        { chunkName: "mermaid-export", pattern: "apps/web/src/features/graph/mermaid-export*" },

        { chunkName: "ollama", pattern: "packages/api/src/ollama/**" },
        { chunkName: "embeddings", pattern: "packages/db/src/repository/semantic.ts" },
        { chunkName: "semantic-search", pattern: "packages/api/src/chunks/routes.ts" },
        { chunkName: "semantic-neighbors", pattern: "apps/web/src/features/chunks/detail/chunk-neighbors.tsx" },
        { chunkName: "enrichment", pattern: "packages/api/src/enrich/**" },
        { chunkName: "context-export", pattern: "packages/api/src/context-export/**" },

        { chunkName: "chunks-concept", pattern: "packages/db/src/schema/chunk.ts" },
        { chunkName: "connections-concept", pattern: "packages/db/src/schema/chunk.ts" },
        { chunkName: "tags-concept", pattern: "packages/db/src/schema/tag.ts" },
        { chunkName: "health-scores", pattern: "packages/api/src/chunks/health-score.ts" },
        { chunkName: "staleness-detection", pattern: "packages/db/src/schema/staleness.ts" },
        { chunkName: "applies-to-refs", pattern: "packages/db/src/schema/applies-to.ts" },
        { chunkName: "applies-to-refs", pattern: "packages/db/src/schema/file-ref.ts" },
        { chunkName: "decision-context", pattern: "packages/db/src/schema/chunk.ts" },

        { chunkName: "plans-overview", pattern: "packages/db/src/schema/plan.ts" },
        { chunkName: "plans-overview", pattern: "apps/web/src/routes/plans.*.tsx" },
        { chunkName: "analyze-items", pattern: "packages/api/src/plans/analyze.ts" },
        { chunkName: "task-dependencies", pattern: "packages/db/src/repository/plan.ts" },
        { chunkName: "plan-metadata", pattern: "packages/db/src/schema/plan.ts" },
        { chunkName: "external-links", pattern: "packages/db/src/schema/plan.ts" },
        { chunkName: "activity-audit", pattern: "packages/api/src/plans/**" },
        { chunkName: "requirements-bdd", pattern: "packages/db/src/schema/requirement.ts" },

        { chunkName: "mcp-server", pattern: "packages/mcp/**" },
        { chunkName: "vscode-ext", pattern: "apps/vscode/**" },
        { chunkName: "cli-overview", pattern: "apps/cli/**" },
        { chunkName: "claude-md-sync", pattern: "apps/cli/src/lib/**" },
        { chunkName: "git-hooks", pattern: "apps/cli/src/commands/hooks.ts" },

        { chunkName: "seed-system", pattern: "packages/db/src/seed/**" }
    ];

    const appliesRows = applies
        .filter(a => !!c[a.chunkName])
        .map(a => ({ id: uuid(), chunkId: c[a.chunkName]!, pattern: a.pattern, note: a.note ?? null }));
    if (appliesRows.length > 0) {
        await ctx.db.insert(chunkAppliesTo).values(appliesRows);
        ctx.counters["applies_to"] = appliesRows.length;
    }

    const refs: Array<{ chunkName: string; path: string; relation: string }> = [
        { chunkName: "seed-system", path: "packages/db/src/seed/index.ts", relation: "documents" },
        { chunkName: "seed-system", path: "packages/db/src/seed/modules/core.ts", relation: "documents" },
        { chunkName: "repo-pattern", path: "packages/api/src/chunks/service.ts", relation: "documents" },
        { chunkName: "catalog-pattern", path: "packages/db/src/schema/chunk-type.ts", relation: "documents" },
        { chunkName: "graph-perf", path: "apps/web/src/features/graph/graph-timings.ts", relation: "documents" },
        { chunkName: "graph-perf", path: "apps/web/src/features/graph/layout-cache.ts", relation: "documents" },
        { chunkName: "graph-layout", path: "apps/web/src/features/graph/force-layout.ts", relation: "documents" },
        { chunkName: "mermaid-export", path: "apps/web/src/features/graph/mermaid-export.ts", relation: "documents" },
        { chunkName: "plan-metadata", path: "packages/db/src/schema/plan.ts", relation: "documents" },
        { chunkName: "external-links", path: "packages/db/src/schema/plan.ts", relation: "documents" }
    ];

    const refRows = refs
        .filter(r => !!c[r.chunkName])
        .map(r => ({ id: uuid(), chunkId: c[r.chunkName]!, path: r.path, anchor: null, relation: r.relation }));
    if (refRows.length > 0) {
        await ctx.db.insert(chunkFileRef).values(refRows);
        ctx.counters["file_refs"] = refRows.length;
    }
}

// No reset — cascade from chunks.
