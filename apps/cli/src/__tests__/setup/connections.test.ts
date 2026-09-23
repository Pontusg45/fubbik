import { describe, expect, it } from "vitest";

import { inferConnections } from "../../lib/setup/connections";
import type { DiscoveredChunk } from "../../lib/setup/types";

function makeChunk(overrides: Partial<DiscoveredChunk> & { title: string }): DiscoveredChunk {
    return {
        content: "",
        type: "reference",
        tags: [],
        tier: 1,
        category: "documents",
        source: "test",
        ...overrides
    };
}

describe("inferConnections", () => {
    it("creates references connections for markdown links between tier-1 docs", () => {
        // Given
        const readme = makeChunk({
            title: "README",
            content: "See the [Guide](./docs/guide.md) for more info.",
            tier: 1,
            source: "README.md"
        });
        const guide = makeChunk({
            title: "Guide",
            tier: 1,
            source: "docs/guide.md"
        });

        const connections = inferConnections([readme, guide]);

        // When
        const ref = connections.find(c => c.sourceTitle === "README" && c.targetTitle === "Guide" && c.relation === "references");
        // Then
        expect(ref).toBeDefined();
    });

    it("creates part_of connections for monorepo packages", () => {
        // Given
        const structure = makeChunk({
            title: "Project Structure (Monorepo)",
            tier: 2,
            category: "structure",
            source: "package.json"
        });
        const techStack = makeChunk({
            title: "Tech Stack — @mono/web",
            tier: 2,
            category: "tech-stack",
            source: "apps/web/package.json"
        });

        const connections = inferConnections([structure, techStack]);

        // When
        const partOf = connections.find(
            c => c.sourceTitle === "Tech Stack — @mono/web" && c.targetTitle === "Project Structure (Monorepo)" && c.relation === "part_of"
        );
        // Then
        expect(partOf).toBeDefined();
    });

    it("creates depends_on between routes and database", () => {
        // Given
        const routes = makeChunk({
            title: "API Routes",
            tier: 3,
            tags: ["routing"],
            source: "src/routes"
        });
        const db = makeChunk({
            title: "Database Schema",
            tier: 3,
            tags: ["database"],
            source: "src/db"
        });

        const connections = inferConnections([routes, db]);

        // When
        const dep = connections.find(
            c => c.sourceTitle === "API Routes" && c.targetTitle === "Database Schema" && c.relation === "depends_on"
        );
        // Then
        expect(dep).toBeDefined();
    });

    it("creates supports connections when tier2/3 keywords appear in tier1 content", () => {
        // Given
        const readme = makeChunk({
            title: "README",
            content: "This project uses Drizzle ORM for database access",
            tier: 1,
            source: "README.md"
        });
        const dbSchema = makeChunk({
            title: "Database Schema",
            content: "drizzle-orm",
            tier: 3,
            tags: ["database"],
            source: "src/db"
        });

        const connections = inferConnections([readme, dbSchema]);

        // When
        const supports = connections.find(
            c => c.sourceTitle === "Database Schema" && c.targetTitle === "README" && c.relation === "supports"
        );
        // Then
        expect(supports).toBeDefined();
    });

    it("returns empty for unrelated chunks", () => {
        // Given
        const readme = makeChunk({
            title: "README",
            content: "A simple hello world project",
            tier: 1,
            source: "README.md"
        });
        const ci = makeChunk({
            title: "CI Pipeline",
            tier: 2,
            category: "config",
            tags: ["ci"],
            source: ".github/workflows/ci.yml"
        });

        const connections = inferConnections([readme, ci]);

        // When
        const supports = connections.filter(c => c.relation === "supports");
        // Then
        expect(supports).toHaveLength(0);
    });
});
