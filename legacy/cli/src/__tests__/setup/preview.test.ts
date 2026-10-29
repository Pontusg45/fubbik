import { describe, expect, it } from "vitest";

import { formatPreview } from "../../lib/setup/preview";
import type { DiscoveredChunk, DiscoveredConnection } from "../../lib/setup/types";

function makeChunk(overrides: Partial<DiscoveredChunk> & { title: string; category: DiscoveredChunk["category"] }): DiscoveredChunk {
    return { content: "", type: "reference", tags: [], tier: 1, source: "test", ...overrides };
}

describe("formatPreview", () => {
    it("groups chunks by category with counts", () => {
        // Given
        const chunks: DiscoveredChunk[] = [
            makeChunk({ title: "README", category: "documents" }),
            makeChunk({ title: "Guide", category: "documents" }),
            makeChunk({ title: "Tech Stack", category: "tech-stack" })
        ];
        // When
        const output = formatPreview(chunks, [], []);
        // Then
        expect(output).toContain("Documents");
        expect(output).toContain("2");
        expect(output).toContain("Tech stack");
        expect(output).toContain("1");
        expect(output).toContain("3 chunks");
    });

    it("shows connection count", () => {
        // Given
        const chunks = [makeChunk({ title: "A", category: "documents" })];
        const connections: DiscoveredConnection[] = [
            { sourceTitle: "A", targetTitle: "B", relation: "references" },
            { sourceTitle: "C", targetTitle: "D", relation: "depends_on" }
        ];
        // When
        const output = formatPreview(chunks, connections, []);
        // Then
        expect(output).toContain("2 connections");
    });

    it("shows example titles per category", () => {
        // Given
        const chunks = [
            makeChunk({ title: "README", category: "documents" }),
            makeChunk({ title: "Contributing Guide", category: "documents" })
        ];
        // When
        const output = formatPreview(chunks, [], []);
        // Then
        expect(output).toContain("README");
    });
});
