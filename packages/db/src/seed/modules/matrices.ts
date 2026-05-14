import { eq } from "drizzle-orm";

import {
    behaviorMatrix,
    behaviorDimension,
    behaviorRule,
    behaviorCell,
    behaviorCellRequirement
} from "../../schema/behavior-matrix";
import { uuid } from "../factories";
import type { SeedContext } from "../context";

// Helper to batch-insert dimensions, rules, and cells
async function insertDimensions(ctx: SeedContext, matrixId: string, dims: Array<{ name: string; key: string }>) {
    for (let i = 0; i < dims.length; i++) {
        const dim = dims[i]!;
        const id = uuid();
        await ctx.db.insert(behaviorDimension).values({ id, matrixId, name: dim.name, order: i });
        ctx.ids.matrixDimensions[dim.key] = id;
    }
    return dims.length;
}

async function insertRules(ctx: SeedContext, matrixId: string, rules: Array<{ title: string; category: string; key: string; description?: string }>) {
    for (let i = 0; i < rules.length; i++) {
        const rule = rules[i]!;
        const id = uuid();
        await ctx.db.insert(behaviorRule).values({ id, matrixId, title: rule.title, description: rule.description, category: rule.category, order: i });
        ctx.ids.matrixRules[rule.key] = id;
    }
    return rules.length;
}

async function insertCells(ctx: SeedContext, cellDefs: Array<{ key: string; rule: string; dim: string }>) {
    for (const c of cellDefs) {
        const ruleId = ctx.ids.matrixRules[c.rule];
        const dimId = ctx.ids.matrixDimensions[c.dim];
        if (!ruleId || !dimId) throw new Error(`missing rule "${c.rule}" or dim "${c.dim}"`);
        const id = uuid();
        await ctx.db.insert(behaviorCell).values({ id, ruleId, dimensionId: dimId });
        ctx.ids.matrixCells[c.key] = id;
    }
    return cellDefs.length;
}

export async function seed(ctx: SeedContext): Promise<void> {
    const codebaseId = ctx.ids.codebases["fubbik"];
    if (!codebaseId) throw new Error("matrices needs fubbik codebase");

    let totalDims = 0;
    let totalRules = 0;
    let totalCells = 0;

    // ═══════════════════════════════════════════════════════════════════
    // MATRIX 1: Domain Invariants (rules × entities)
    // ═══════════════════════════════════════════════════════════════════

    const invariantId = uuid();
    await ctx.db.insert(behaviorMatrix).values({
        id: invariantId,
        name: "Domain Invariants",
        layer: "invariant",
        description: "Rules that must always hold across domain entities — data integrity, ownership, lifecycle, and structural constraints",
        codebaseId,
        userId: ctx.userId
    });
    ctx.ids.matrices["invariants"] = invariantId;

    totalDims += await insertDimensions(ctx, invariantId, [
        { name: "Chunk", key: "e-chunk" },
        { name: "Connection", key: "e-connection" },
        { name: "Tag", key: "e-tag" },
        { name: "Plan", key: "e-plan" },
        { name: "Plan Task", key: "e-task" },
        { name: "Requirement", key: "e-requirement" },
        { name: "Feature", key: "e-feature" },
        { name: "Codebase", key: "e-codebase" },
        { name: "Workspace", key: "e-workspace" },
        { name: "Template", key: "e-template" },
        { name: "Collection", key: "e-collection" },
        { name: "Document", key: "e-document" },
        { name: "Vocabulary", key: "e-vocabulary" },
        { name: "Scope Key", key: "e-scope-key" }
    ]);

    totalRules += await insertRules(ctx, invariantId, [
        // --- Validation ---
        { title: "Title/name is required", category: "Validation", key: "r-title-required" },
        { title: "Content/body is required", category: "Validation", key: "r-content-required" },
        { title: "BDD steps follow given→when→then order", category: "Validation", key: "r-bdd-order",
          description: "Steps must start with given, include at least one when and one then, with no backtracking" },
        { title: "Unique name per user", category: "Validation", key: "r-unique-name" },
        { title: "Unique source+target+relation", category: "Validation", key: "r-unique-edge" },

        // --- Ownership & Auth ---
        { title: "Must belong to a user", category: "Ownership", key: "r-user-owned" },
        { title: "Only owner can mutate", category: "Ownership", key: "r-owner-write" },
        { title: "Builtin entries are read-only", category: "Ownership", key: "r-builtin-readonly",
          description: "Builtin templates, chunk types, and connection relations cannot be modified or deleted" },

        // --- Lifecycle ---
        { title: "Cascade deletes to children", category: "Lifecycle", key: "r-cascade-delete",
          description: "Deleting a parent removes all child entities via FK cascade" },
        { title: "Changes create version history", category: "Lifecycle", key: "r-version-history" },
        { title: "Supports archive/restore", category: "Lifecycle", key: "r-archivable" },
        { title: "Status transitions are ungated", category: "Lifecycle", key: "r-status-ungated",
          description: "Status labels only — no enforced state machine. Any status can transition to any other." },
        { title: "Marking task done unblocks dependents", category: "Lifecycle", key: "r-task-unblock" },

        // --- Data Integrity ---
        { title: "Optional codebase scoping", category: "Data Integrity", key: "r-codebase-scope",
          description: "Can belong to a codebase or be global (null codebase)" },
        { title: "Embedding auto-refreshes on content change", category: "Data Integrity", key: "r-embedding-refresh" },
        { title: "Ordering preserved via order column", category: "Data Integrity", key: "r-ordering" },
        { title: "Priority is unique per user", category: "Data Integrity", key: "r-priority-unique" },
        { title: "Feature deltas are sparse JSONB", category: "Data Integrity", key: "r-delta-sparse",
          description: "Deltas contain only changed fields, not full copies" },
        { title: "Merge permanently applies deltas to base", category: "Data Integrity", key: "r-merge-permanent" }
    ]);

    // Build cells for invariant matrix
    // Each entry: which rule applies to which entity (intentional gaps = not relevant)
    totalCells += await insertCells(ctx, [
        // Title/name is required
        { key: "c-title-chunk",      rule: "r-title-required",    dim: "e-chunk" },
        { key: "c-title-plan",       rule: "r-title-required",    dim: "e-plan" },
        { key: "c-title-task",       rule: "r-title-required",    dim: "e-task" },
        { key: "c-title-req",        rule: "r-title-required",    dim: "e-requirement" },
        { key: "c-title-feature",    rule: "r-title-required",    dim: "e-feature" },
        { key: "c-title-codebase",   rule: "r-title-required",    dim: "e-codebase" },
        { key: "c-title-workspace",  rule: "r-title-required",    dim: "e-workspace" },
        { key: "c-title-template",   rule: "r-title-required",    dim: "e-template" },
        { key: "c-title-collection", rule: "r-title-required",    dim: "e-collection" },
        { key: "c-title-document",   rule: "r-title-required",    dim: "e-document" },
        { key: "c-title-tag",        rule: "r-title-required",    dim: "e-tag" },

        // Content/body is required
        { key: "c-content-chunk",    rule: "r-content-required",  dim: "e-chunk" },
        { key: "c-content-template", rule: "r-content-required",  dim: "e-template" },

        // BDD step ordering
        { key: "c-bdd-req",          rule: "r-bdd-order",         dim: "e-requirement" },

        // Unique name per user
        { key: "c-uniq-tag",         rule: "r-unique-name",       dim: "e-tag" },
        { key: "c-uniq-codebase",    rule: "r-unique-name",       dim: "e-codebase" },
        { key: "c-uniq-workspace",   rule: "r-unique-name",       dim: "e-workspace" },
        { key: "c-uniq-feature",     rule: "r-unique-name",       dim: "e-feature" },
        { key: "c-uniq-collection",  rule: "r-unique-name",       dim: "e-collection" },

        // Unique source+target+relation
        { key: "c-uniq-connection",  rule: "r-unique-edge",       dim: "e-connection" },

        // Must belong to a user
        { key: "c-owned-chunk",      rule: "r-user-owned",        dim: "e-chunk" },
        { key: "c-owned-plan",       rule: "r-user-owned",        dim: "e-plan" },
        { key: "c-owned-req",        rule: "r-user-owned",        dim: "e-requirement" },
        { key: "c-owned-feature",    rule: "r-user-owned",        dim: "e-feature" },
        { key: "c-owned-codebase",   rule: "r-user-owned",        dim: "e-codebase" },
        { key: "c-owned-workspace",  rule: "r-user-owned",        dim: "e-workspace" },
        { key: "c-owned-tag",        rule: "r-user-owned",        dim: "e-tag" },
        { key: "c-owned-collection", rule: "r-user-owned",        dim: "e-collection" },
        { key: "c-owned-document",   rule: "r-user-owned",        dim: "e-document" },
        { key: "c-owned-scope-key",  rule: "r-user-owned",        dim: "e-scope-key" },

        // Only owner can mutate
        { key: "c-write-chunk",      rule: "r-owner-write",       dim: "e-chunk" },
        { key: "c-write-plan",       rule: "r-owner-write",       dim: "e-plan" },
        { key: "c-write-req",        rule: "r-owner-write",       dim: "e-requirement" },
        { key: "c-write-feature",    rule: "r-owner-write",       dim: "e-feature" },
        { key: "c-write-codebase",   rule: "r-owner-write",       dim: "e-codebase" },

        // Builtin entries are read-only
        { key: "c-builtin-template", rule: "r-builtin-readonly",  dim: "e-template" },

        // Cascade deletes to children
        { key: "c-cascade-plan",     rule: "r-cascade-delete",    dim: "e-plan" },
        { key: "c-cascade-feature",  rule: "r-cascade-delete",    dim: "e-feature" },
        { key: "c-cascade-codebase", rule: "r-cascade-delete",    dim: "e-codebase" },
        { key: "c-cascade-workspace",rule: "r-cascade-delete",    dim: "e-workspace" },
        { key: "c-cascade-document", rule: "r-cascade-delete",    dim: "e-document" },

        // Version history
        { key: "c-version-chunk",    rule: "r-version-history",   dim: "e-chunk" },

        // Archive/restore
        { key: "c-archive-chunk",    rule: "r-archivable",        dim: "e-chunk" },
        { key: "c-archive-plan",     rule: "r-archivable",        dim: "e-plan" },

        // Status transitions ungated
        { key: "c-status-plan",      rule: "r-status-ungated",    dim: "e-plan" },
        { key: "c-status-task",      rule: "r-status-ungated",    dim: "e-task" },
        { key: "c-status-req",       rule: "r-status-ungated",    dim: "e-requirement" },

        // Task done unblocks dependents
        { key: "c-unblock-task",     rule: "r-task-unblock",      dim: "e-task" },

        // Optional codebase scoping
        { key: "c-scope-chunk",      rule: "r-codebase-scope",    dim: "e-chunk" },
        { key: "c-scope-plan",       rule: "r-codebase-scope",    dim: "e-plan" },
        { key: "c-scope-req",        rule: "r-codebase-scope",    dim: "e-requirement" },
        { key: "c-scope-collection", rule: "r-codebase-scope",    dim: "e-collection" },
        { key: "c-scope-document",   rule: "r-codebase-scope",    dim: "e-document" },
        { key: "c-scope-vocabulary", rule: "r-codebase-scope",    dim: "e-vocabulary" },

        // Embedding auto-refresh
        { key: "c-embed-chunk",      rule: "r-embedding-refresh", dim: "e-chunk" },

        // Ordering preserved
        { key: "c-order-task",       rule: "r-ordering",          dim: "e-task" },
        { key: "c-order-req",        rule: "r-ordering",          dim: "e-requirement" },

        // Priority unique per user
        { key: "c-priority-feature", rule: "r-priority-unique",   dim: "e-feature" },

        // Delta sparse JSONB
        { key: "c-delta-feature",    rule: "r-delta-sparse",      dim: "e-feature" },

        // Merge permanent
        { key: "c-merge-feature",    rule: "r-merge-permanent",   dim: "e-feature" }
    ]);

    // ═══════════════════════════════════════════════════════════════════
    // MATRIX 2: API Contracts (capabilities × actors)
    // ═══════════════════════════════════════════════════════════════════

    const contractId = uuid();
    await ctx.db.insert(behaviorMatrix).values({
        id: contractId,
        name: "API Contracts",
        layer: "contract",
        description: "Which capabilities are available to each actor — Web User, AI Agent (MCP), CLI, API Consumer, System (background)",
        codebaseId,
        userId: ctx.userId
    });
    ctx.ids.matrices["contracts"] = contractId;

    totalDims += await insertDimensions(ctx, contractId, [
        { name: "Web User", key: "a-web" },
        { name: "AI Agent (MCP)", key: "a-mcp" },
        { name: "CLI User", key: "a-cli" },
        { name: "API Consumer", key: "a-api" },
        { name: "System", key: "a-system" }
    ]);

    totalRules += await insertRules(ctx, contractId, [
        // --- Chunk capabilities ---
        { title: "Chunk CRUD", category: "Chunks", key: "cap-chunk-crud" },
        { title: "Chunk search (full-text)", category: "Chunks", key: "cap-chunk-search" },
        { title: "Chunk search (semantic/embedding)", category: "Chunks", key: "cap-chunk-semantic" },
        { title: "Chunk search (federated cross-codebase)", category: "Chunks", key: "cap-chunk-federated" },
        { title: "Chunk bulk operations", category: "Chunks", key: "cap-chunk-bulk" },
        { title: "Chunk import (JSON/markdown/directory)", category: "Chunks", key: "cap-chunk-import" },
        { title: "Chunk export", category: "Chunks", key: "cap-chunk-export" },
        { title: "Chunk AI enrichment (summary, aliases)", category: "Chunks", key: "cap-chunk-enrich" },
        { title: "Chunk review workflow", category: "Chunks", key: "cap-chunk-review" },
        { title: "Chunk proposal workflow", category: "Chunks", key: "cap-chunk-proposal" },
        { title: "Chunk health scoring", category: "Chunks", key: "cap-chunk-health" },
        { title: "Chunk similarity detection", category: "Chunks", key: "cap-chunk-similar" },

        // --- Connection capabilities ---
        { title: "Connection CRUD", category: "Connections", key: "cap-conn-crud" },
        { title: "Connection suggestions (graph-based)", category: "Connections", key: "cap-conn-suggest" },

        // --- Tag capabilities ---
        { title: "Tag CRUD", category: "Tags", key: "cap-tag-crud" },
        { title: "Tag merge", category: "Tags", key: "cap-tag-merge" },
        { title: "Tag suggestions (neighbor frequency)", category: "Tags", key: "cap-tag-suggest" },

        // --- Requirement capabilities ---
        { title: "Requirement CRUD", category: "Requirements", key: "cap-req-crud" },
        { title: "Requirement BDD export (Gherkin/Vitest)", category: "Requirements", key: "cap-req-export" },
        { title: "Requirement AI generation", category: "Requirements", key: "cap-req-ai-gen",
          description: "AI generates requirements from context analysis" },
        { title: "Requirement coverage analysis", category: "Requirements", key: "cap-req-coverage" },

        // --- Plan capabilities ---
        { title: "Plan CRUD", category: "Plans", key: "cap-plan-crud" },
        { title: "Plan task management", category: "Plans", key: "cap-plan-tasks" },
        { title: "Plan analyze items (risks, assumptions)", category: "Plans", key: "cap-plan-analyze" },
        { title: "Plan-requirement linking", category: "Plans", key: "cap-plan-req-link" },

        // --- Feature capabilities ---
        { title: "Feature overlay management", category: "Features", key: "cap-feature-crud" },
        { title: "Feature delta editing", category: "Features", key: "cap-feature-delta" },
        { title: "Feature merge to base", category: "Features", key: "cap-feature-merge" },

        // --- Context capabilities ---
        { title: "Context retrieval (for file/plan/concept)", category: "Context", key: "cap-context-get" },
        { title: "Context snapshot creation", category: "Context", key: "cap-context-snapshot" },
        { title: "CLAUDE.md generation from tags", category: "Context", key: "cap-claudemd" },

        // --- Graph & Analysis ---
        { title: "Graph visualization", category: "Analysis", key: "cap-graph-viz" },
        { title: "Graph path finding", category: "Analysis", key: "cap-graph-paths" },
        { title: "Cluster analysis", category: "Analysis", key: "cap-cluster" },
        { title: "Density analysis", category: "Analysis", key: "cap-density" },
        { title: "Knowledge health dashboard", category: "Analysis", key: "cap-health-dash" },
        { title: "Staleness detection and scanning", category: "Analysis", key: "cap-staleness" },

        // --- Organization ---
        { title: "Codebase management", category: "Organization", key: "cap-codebase" },
        { title: "Workspace management", category: "Organization", key: "cap-workspace" },
        { title: "Collection management", category: "Organization", key: "cap-collection" },
        { title: "Template management", category: "Organization", key: "cap-template" },
        { title: "Vocabulary management", category: "Organization", key: "cap-vocabulary" },
        { title: "Scope key registry", category: "Organization", key: "cap-scope-key" },

        // --- Matrix capabilities ---
        { title: "Behavioral matrix management", category: "Matrices", key: "cap-matrix-crud" },
        { title: "Matrix gap analysis", category: "Matrices", key: "cap-matrix-gaps" },

        // --- System capabilities ---
        { title: "Activity logging", category: "System", key: "cap-activity" },
        { title: "Notifications", category: "System", key: "cap-notifications" },
        { title: "Settings management (user/codebase/instance)", category: "System", key: "cap-settings" },
        { title: "Favorites and saved queries", category: "System", key: "cap-favorites" },
        { title: "Learning paths", category: "System", key: "cap-learning-paths" }
    ]);

    // Build cells — which actors can do what
    // Intentional gaps create unspecified cells visible in the matrix
    totalCells += await insertCells(ctx, [
        // --- Chunk CRUD: all actors ---
        { key: "cc-chunk-web",     rule: "cap-chunk-crud",       dim: "a-web" },
        { key: "cc-chunk-mcp",     rule: "cap-chunk-crud",       dim: "a-mcp" },
        { key: "cc-chunk-cli",     rule: "cap-chunk-crud",       dim: "a-cli" },
        { key: "cc-chunk-api",     rule: "cap-chunk-crud",       dim: "a-api" },

        // --- Chunk search (text): all except system ---
        { key: "cc-search-web",    rule: "cap-chunk-search",     dim: "a-web" },
        { key: "cc-search-mcp",    rule: "cap-chunk-search",     dim: "a-mcp" },
        { key: "cc-search-cli",    rule: "cap-chunk-search",     dim: "a-cli" },
        { key: "cc-search-api",    rule: "cap-chunk-search",     dim: "a-api" },

        // --- Semantic search: web, MCP, CLI ---
        { key: "cc-semantic-web",  rule: "cap-chunk-semantic",   dim: "a-web" },
        { key: "cc-semantic-mcp",  rule: "cap-chunk-semantic",   dim: "a-mcp" },
        { key: "cc-semantic-cli",  rule: "cap-chunk-semantic",   dim: "a-cli" },

        // --- Federated search: web, CLI, API ---
        { key: "cc-fed-web",      rule: "cap-chunk-federated",   dim: "a-web" },
        { key: "cc-fed-cli",      rule: "cap-chunk-federated",   dim: "a-cli" },
        { key: "cc-fed-api",      rule: "cap-chunk-federated",   dim: "a-api" },

        // --- Bulk operations: web, API ---
        { key: "cc-bulk-web",      rule: "cap-chunk-bulk",       dim: "a-web" },
        { key: "cc-bulk-api",      rule: "cap-chunk-bulk",       dim: "a-api" },

        // --- Import: web, CLI ---
        { key: "cc-import-web",    rule: "cap-chunk-import",     dim: "a-web" },
        { key: "cc-import-cli",    rule: "cap-chunk-import",     dim: "a-cli" },

        // --- Export: web, CLI, API ---
        { key: "cc-export-web",    rule: "cap-chunk-export",     dim: "a-web" },
        { key: "cc-export-cli",    rule: "cap-chunk-export",     dim: "a-cli" },
        { key: "cc-export-api",    rule: "cap-chunk-export",     dim: "a-api" },

        // --- Enrichment: web, system (auto on create/update) ---
        { key: "cc-enrich-web",    rule: "cap-chunk-enrich",     dim: "a-web" },
        { key: "cc-enrich-api",    rule: "cap-chunk-enrich",     dim: "a-api" },
        { key: "cc-enrich-sys",    rule: "cap-chunk-enrich",     dim: "a-system" },

        // --- Review workflow: web, API ---
        { key: "cc-review-web",    rule: "cap-chunk-review",     dim: "a-web" },
        { key: "cc-review-api",    rule: "cap-chunk-review",     dim: "a-api" },

        // --- Proposal workflow: web, MCP ---
        { key: "cc-proposal-web",  rule: "cap-chunk-proposal",   dim: "a-web" },
        { key: "cc-proposal-mcp",  rule: "cap-chunk-proposal",   dim: "a-mcp" },

        // --- Health scoring: web, API ---
        { key: "cc-health-web",    rule: "cap-chunk-health",     dim: "a-web" },
        { key: "cc-health-api",    rule: "cap-chunk-health",     dim: "a-api" },

        // --- Similarity: web, API ---
        { key: "cc-similar-web",   rule: "cap-chunk-similar",    dim: "a-web" },
        { key: "cc-similar-api",   rule: "cap-chunk-similar",    dim: "a-api" },

        // --- Connection CRUD: web, CLI, API ---
        { key: "cc-conn-web",      rule: "cap-conn-crud",        dim: "a-web" },
        { key: "cc-conn-cli",      rule: "cap-conn-crud",        dim: "a-cli" },
        { key: "cc-conn-api",      rule: "cap-conn-crud",        dim: "a-api" },

        // --- Connection suggest: web, API ---
        { key: "cc-connsug-web",   rule: "cap-conn-suggest",     dim: "a-web" },
        { key: "cc-connsug-api",   rule: "cap-conn-suggest",     dim: "a-api" },

        // --- Tag CRUD: web, CLI, API ---
        { key: "cc-tag-web",       rule: "cap-tag-crud",         dim: "a-web" },
        { key: "cc-tag-cli",       rule: "cap-tag-crud",         dim: "a-cli" },
        { key: "cc-tag-api",       rule: "cap-tag-crud",         dim: "a-api" },

        // --- Tag merge: web ---
        { key: "cc-tagmerge-web",  rule: "cap-tag-merge",        dim: "a-web" },

        // --- Tag suggestions: web, API ---
        { key: "cc-tagsug-web",    rule: "cap-tag-suggest",      dim: "a-web" },
        { key: "cc-tagsug-api",    rule: "cap-tag-suggest",      dim: "a-api" },

        // --- Requirement CRUD: web, MCP, CLI, API ---
        { key: "cc-req-web",       rule: "cap-req-crud",         dim: "a-web" },
        { key: "cc-req-mcp",       rule: "cap-req-crud",         dim: "a-mcp" },
        { key: "cc-req-cli",       rule: "cap-req-crud",         dim: "a-cli" },
        { key: "cc-req-api",       rule: "cap-req-crud",         dim: "a-api" },

        // --- Requirement export: web, CLI, API ---
        { key: "cc-reqexp-web",    rule: "cap-req-export",       dim: "a-web" },
        { key: "cc-reqexp-cli",    rule: "cap-req-export",       dim: "a-cli" },
        { key: "cc-reqexp-api",    rule: "cap-req-export",       dim: "a-api" },

        // --- Requirement AI generation: MCP only ---
        { key: "cc-reqai-mcp",     rule: "cap-req-ai-gen",       dim: "a-mcp" },

        // --- Requirement coverage: web, API ---
        { key: "cc-reqcov-web",    rule: "cap-req-coverage",     dim: "a-web" },
        { key: "cc-reqcov-api",    rule: "cap-req-coverage",     dim: "a-api" },

        // --- Plan CRUD: web, MCP, CLI, API ---
        { key: "cc-plan-web",      rule: "cap-plan-crud",        dim: "a-web" },
        { key: "cc-plan-mcp",      rule: "cap-plan-crud",        dim: "a-mcp" },
        { key: "cc-plan-cli",      rule: "cap-plan-crud",        dim: "a-cli" },
        { key: "cc-plan-api",      rule: "cap-plan-crud",        dim: "a-api" },

        // --- Plan tasks: web, MCP, CLI, API ---
        { key: "cc-task-web",      rule: "cap-plan-tasks",       dim: "a-web" },
        { key: "cc-task-mcp",      rule: "cap-plan-tasks",       dim: "a-mcp" },
        { key: "cc-task-cli",      rule: "cap-plan-tasks",       dim: "a-cli" },
        { key: "cc-task-api",      rule: "cap-plan-tasks",       dim: "a-api" },

        // --- Plan analyze: web, MCP, API ---
        { key: "cc-analyze-web",   rule: "cap-plan-analyze",     dim: "a-web" },
        { key: "cc-analyze-mcp",   rule: "cap-plan-analyze",     dim: "a-mcp" },
        { key: "cc-analyze-api",   rule: "cap-plan-analyze",     dim: "a-api" },

        // --- Plan-requirement linking: web, MCP, CLI, API ---
        { key: "cc-planreq-web",   rule: "cap-plan-req-link",    dim: "a-web" },
        { key: "cc-planreq-mcp",   rule: "cap-plan-req-link",    dim: "a-mcp" },
        { key: "cc-planreq-cli",   rule: "cap-plan-req-link",    dim: "a-cli" },
        { key: "cc-planreq-api",   rule: "cap-plan-req-link",    dim: "a-api" },

        // --- Feature overlay: web, API ---
        { key: "cc-feat-web",      rule: "cap-feature-crud",     dim: "a-web" },
        { key: "cc-feat-api",      rule: "cap-feature-crud",     dim: "a-api" },

        // --- Feature deltas: web, API ---
        { key: "cc-delta-web",     rule: "cap-feature-delta",    dim: "a-web" },
        { key: "cc-delta-api",     rule: "cap-feature-delta",    dim: "a-api" },

        // --- Feature merge: web, API ---
        { key: "cc-fmerge-web",    rule: "cap-feature-merge",    dim: "a-web" },
        { key: "cc-fmerge-api",    rule: "cap-feature-merge",    dim: "a-api" },

        // --- Context retrieval: web, MCP, CLI, API ---
        { key: "cc-ctx-web",       rule: "cap-context-get",      dim: "a-web" },
        { key: "cc-ctx-mcp",       rule: "cap-context-get",      dim: "a-mcp" },
        { key: "cc-ctx-cli",       rule: "cap-context-get",      dim: "a-cli" },
        { key: "cc-ctx-api",       rule: "cap-context-get",      dim: "a-api" },

        // --- Context snapshot: web, MCP ---
        { key: "cc-snap-web",      rule: "cap-context-snapshot",  dim: "a-web" },
        { key: "cc-snap-mcp",      rule: "cap-context-snapshot",  dim: "a-mcp" },

        // --- CLAUDE.md: CLI, MCP ---
        { key: "cc-claude-cli",    rule: "cap-claudemd",         dim: "a-cli" },
        { key: "cc-claude-mcp",    rule: "cap-claudemd",         dim: "a-mcp" },

        // --- Graph viz: web only ---
        { key: "cc-graph-web",     rule: "cap-graph-viz",        dim: "a-web" },

        // --- Graph paths: web, API ---
        { key: "cc-paths-web",     rule: "cap-graph-paths",      dim: "a-web" },
        { key: "cc-paths-api",     rule: "cap-graph-paths",      dim: "a-api" },

        // --- Cluster analysis: web, API ---
        { key: "cc-cluster-web",   rule: "cap-cluster",          dim: "a-web" },
        { key: "cc-cluster-api",   rule: "cap-cluster",          dim: "a-api" },

        // --- Density: web, API ---
        { key: "cc-density-web",   rule: "cap-density",          dim: "a-web" },
        { key: "cc-density-api",   rule: "cap-density",          dim: "a-api" },

        // --- Health dashboard: web ---
        { key: "cc-healthd-web",   rule: "cap-health-dash",      dim: "a-web" },

        // --- Staleness: web, system (auto-scan) ---
        { key: "cc-stale-web",     rule: "cap-staleness",        dim: "a-web" },
        { key: "cc-stale-api",     rule: "cap-staleness",        dim: "a-api" },
        { key: "cc-stale-sys",     rule: "cap-staleness",        dim: "a-system" },

        // --- Codebase mgmt: web, CLI, API ---
        { key: "cc-cb-web",        rule: "cap-codebase",         dim: "a-web" },
        { key: "cc-cb-cli",        rule: "cap-codebase",         dim: "a-cli" },
        { key: "cc-cb-api",        rule: "cap-codebase",         dim: "a-api" },

        // --- Workspace mgmt: web, API ---
        { key: "cc-ws-web",        rule: "cap-workspace",        dim: "a-web" },
        { key: "cc-ws-api",        rule: "cap-workspace",        dim: "a-api" },

        // --- Collection mgmt: web, API ---
        { key: "cc-coll-web",      rule: "cap-collection",       dim: "a-web" },
        { key: "cc-coll-api",      rule: "cap-collection",       dim: "a-api" },

        // --- Template mgmt: web, API ---
        { key: "cc-tmpl-web",      rule: "cap-template",         dim: "a-web" },
        { key: "cc-tmpl-api",      rule: "cap-template",         dim: "a-api" },

        // --- Vocabulary: web, MCP (read-only), CLI, API ---
        { key: "cc-vocab-web",     rule: "cap-vocabulary",       dim: "a-web" },
        { key: "cc-vocab-mcp",     rule: "cap-vocabulary",       dim: "a-mcp" },
        { key: "cc-vocab-cli",     rule: "cap-vocabulary",       dim: "a-cli" },
        { key: "cc-vocab-api",     rule: "cap-vocabulary",       dim: "a-api" },

        // --- Scope keys: web, API ---
        { key: "cc-sk-web",        rule: "cap-scope-key",        dim: "a-web" },
        { key: "cc-sk-api",        rule: "cap-scope-key",        dim: "a-api" },

        // --- Matrix management: web, MCP, CLI, API ---
        { key: "cc-mat-web",       rule: "cap-matrix-crud",      dim: "a-web" },
        { key: "cc-mat-mcp",       rule: "cap-matrix-crud",      dim: "a-mcp" },
        { key: "cc-mat-cli",       rule: "cap-matrix-crud",      dim: "a-cli" },
        { key: "cc-mat-api",       rule: "cap-matrix-crud",      dim: "a-api" },

        // --- Matrix gaps: web, MCP, CLI ---
        { key: "cc-gaps-web",      rule: "cap-matrix-gaps",      dim: "a-web" },
        { key: "cc-gaps-mcp",      rule: "cap-matrix-gaps",      dim: "a-mcp" },
        { key: "cc-gaps-cli",      rule: "cap-matrix-gaps",      dim: "a-cli" },

        // --- Activity: web, system (auto-log) ---
        { key: "cc-activity-web",  rule: "cap-activity",         dim: "a-web" },
        { key: "cc-activity-sys",  rule: "cap-activity",         dim: "a-system" },

        // --- Notifications: web, system (auto-generate) ---
        { key: "cc-notif-web",     rule: "cap-notifications",    dim: "a-web" },
        { key: "cc-notif-sys",     rule: "cap-notifications",    dim: "a-system" },

        // --- Settings: web, API ---
        { key: "cc-settings-web",  rule: "cap-settings",         dim: "a-web" },
        { key: "cc-settings-api",  rule: "cap-settings",         dim: "a-api" },

        // --- Favorites: web, API ---
        { key: "cc-fav-web",       rule: "cap-favorites",        dim: "a-web" },
        { key: "cc-fav-api",       rule: "cap-favorites",        dim: "a-api" },

        // --- Learning paths: web ---
        { key: "cc-lp-web",        rule: "cap-learning-paths",   dim: "a-web" }
    ]);

    // ═══════════════════════════════════════════════════════════════════
    // Link cells to existing requirements for specified/unspecified mix
    // ═══════════════════════════════════════════════════════════════════

    const reqLinks: Array<{ cell: string; req: string }> = [
        { cell: "cc-chunk-web",    req: "onboarding-graph" },
        { cell: "cc-chunk-mcp",    req: "onboarding-graph" },
        { cell: "cc-chunk-cli",    req: "onboarding-graph" },
        { cell: "cc-search-web",   req: "catalog-driven-types" },
        { cell: "cc-search-mcp",   req: "catalog-driven-types" },
        { cell: "c-title-chunk",   req: "onboarding-graph" },
        { cell: "c-owned-chunk",   req: "onboarding-graph" }
    ];

    let linkedCount = 0;
    for (const link of reqLinks) {
        const cellId = ctx.ids.matrixCells[link.cell];
        const requirementId = ctx.ids.requirements[link.req];
        if (!cellId || !requirementId) continue;
        await ctx.db.insert(behaviorCellRequirement).values({ cellId, requirementId });
        linkedCount++;
    }

    ctx.counters["behavior_matrices"] = 2;
    ctx.counters["behavior_dimensions"] = totalDims;
    ctx.counters["behavior_rules"] = totalRules;
    ctx.counters["behavior_cells"] = totalCells;
    ctx.counters["behavior_cell_requirements"] = linkedCount;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(behaviorCellRequirement);
    await ctx.db.delete(behaviorCell);
    await ctx.db.delete(behaviorRule);
    await ctx.db.delete(behaviorDimension);
    await ctx.db.delete(behaviorMatrix).where(eq(behaviorMatrix.userId, ctx.userId));
}
