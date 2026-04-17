/**
 * Connections — the knowledge graph topology.
 *
 * Declared as tuples that read like prose: "arch part_of backend". The loader
 * resolves names to IDs via ctx.ids.chunks. Keep the list grouped by source
 * chunk so the graph structure around any given chunk is visible at a glance.
 */

import { loadConnectionFixtures, type ConnectionFixture } from "../fixtures";
import type { SeedContext } from "../context";

const LINKS: ConnectionFixture[] = [
    // Architecture as the hub
    { from: "arch", to: "schema-chunks", relation: "part_of" },
    { from: "arch", to: "schema-connection", relation: "part_of" },
    { from: "arch", to: "schema-catalogs", relation: "part_of" },
    { from: "arch", to: "schema-plan", relation: "part_of" },
    { from: "arch", to: "repo-pattern", relation: "references" },
    { from: "arch", to: "tanstack-start", relation: "references" },

    // Backend convention cluster
    { from: "repo-pattern", to: "typed-errors", relation: "depends_on" },
    { from: "repo-pattern", to: "schema-chunks", relation: "references" },
    { from: "typed-errors", to: "eden-treaty", relation: "supports" },
    { from: "eden-treaty", to: "tanstack-start", relation: "supports" },
    { from: "env-validation", to: "arch", relation: "part_of" },

    // Data model cross-links
    { from: "schema-chunks", to: "schema-connection", relation: "related_to" },
    { from: "schema-chunks", to: "schema-catalogs", relation: "depends_on" },
    { from: "schema-connection", to: "schema-catalogs", relation: "depends_on" },
    { from: "catalog-pattern", to: "schema-catalogs", relation: "supports" },
    { from: "scope-jsonb", to: "schema-chunks", relation: "part_of" },

    // Frontend pieces
    { from: "tanstack-start", to: "feature-structure", relation: "related_to" },
    { from: "feature-structure", to: "base-ui-render", relation: "references" },
    { from: "react-flow-graph", to: "graph-layout", relation: "depends_on" },
    { from: "react-flow-graph", to: "graph-filter-dialog", relation: "references" },
    { from: "graph-filter-dialog", to: "graph-layout", relation: "references" },
    { from: "graph-perf", to: "react-flow-graph", relation: "references" },
    { from: "graph-perf", to: "graph-layout", relation: "references" },
    { from: "mermaid-export", to: "graph-layout", relation: "references" },

    // AI features
    { from: "embeddings", to: "ollama", relation: "depends_on" },
    { from: "semantic-search", to: "embeddings", relation: "depends_on" },
    { from: "semantic-neighbors", to: "embeddings", relation: "depends_on" },
    { from: "enrichment", to: "ollama", relation: "depends_on" },
    { from: "context-export", to: "semantic-search", relation: "references" },
    { from: "context-export", to: "health-scores", relation: "references" },

    // Knowledge framework conceptual hierarchy
    { from: "chunks-concept", to: "schema-chunks", relation: "references" },
    { from: "connections-concept", to: "schema-connection", relation: "references" },
    { from: "tags-concept", to: "chunks-concept", relation: "related_to" },
    { from: "health-scores", to: "chunks-concept", relation: "part_of" },
    { from: "staleness-detection", to: "health-scores", relation: "related_to" },
    { from: "applies-to-refs", to: "chunks-concept", relation: "part_of" },
    { from: "decision-context", to: "chunks-concept", relation: "part_of" },

    // Plans + requirements cluster
    { from: "plans-overview", to: "schema-plan", relation: "references" },
    { from: "analyze-items", to: "plans-overview", relation: "part_of" },
    { from: "task-dependencies", to: "plans-overview", relation: "part_of" },
    { from: "plan-metadata", to: "plans-overview", relation: "part_of" },
    { from: "external-links", to: "plans-overview", relation: "part_of" },
    { from: "activity-audit", to: "plans-overview", relation: "part_of" },
    { from: "requirements-bdd", to: "plans-overview", relation: "related_to" },

    // Integrations
    { from: "mcp-server", to: "arch", relation: "part_of" },
    { from: "vscode-ext", to: "arch", relation: "part_of" },
    { from: "cli-overview", to: "arch", relation: "part_of" },
    { from: "claude-md-sync", to: "cli-overview", relation: "part_of" },
    { from: "claude-md-sync", to: "context-export", relation: "references" },
    { from: "git-hooks", to: "cli-overview", relation: "part_of" },
    { from: "git-hooks", to: "applies-to-refs", relation: "references" },
    { from: "mcp-server", to: "context-export", relation: "references" },

    // System / meta
    { from: "seed-system", to: "arch", relation: "part_of" },
    { from: "graph-perf", to: "arch", relation: "part_of" },
    { from: "note-conventions", to: "chunks-concept", relation: "related_to" },
    { from: "note-conventions", to: "claude-md-sync", relation: "supports" }
];

export async function seed(ctx: SeedContext): Promise<void> {
    await loadConnectionFixtures(ctx, LINKS);
}

// No reset — connections cascade via FK ON DELETE CASCADE when chunks go.
