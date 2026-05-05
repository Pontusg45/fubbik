import { Effect } from "effect";
import { describe, expect, it, vi, beforeEach } from "vitest";

// Mock repository modules before importing the modules under test
vi.mock("@fubbik/db/repository", () => ({
    listChunks: vi.fn(),
    getTagsForChunks: vi.fn(),
    getChunkConnections: vi.fn(),
    listChunksByTag: vi.fn(),
    // claude-md now also pulls requirements/plans/requirement-chunk links for the
    // "Requirements" section; mock these so the import resolves in tests that
    // only exercise the chunks sections. Each returns an Effect of an empty
    // array / empty shape so the requirements block produces no output.
    listRequirements: vi.fn(() => Effect.succeed({ requirements: [], total: 0 })),
    listPlans: vi.fn(() => Effect.succeed([])),
    getChunksForRequirement: vi.fn(() => Effect.succeed([])),
}));

vi.mock("@fubbik/db/repository/plan", () => ({
    listTasks: vi.fn(() => Effect.succeed([])),
}));

vi.mock("../context/resolvers", () => ({
    enrichChunks: vi.fn(),
    resolveForFiles: vi.fn(),
}));

vi.mock("../context/formatter", () => ({
    formatStructured: vi.fn(),
    formatStructuredMarkdown: vi.fn(),
}));

import {
    listChunks,
    listChunksByTag,
} from "@fubbik/db/repository";
import { enrichChunks, resolveForFiles } from "../context/resolvers";
import { formatStructured, formatStructuredMarkdown } from "../context/formatter";
import { exportContext } from "./service";
import { generateClaudeMd } from "./claude-md";
import type { ChunkWithMetadata } from "../context/formatter";

// ── Helpers ─────────────────────────────────────────────────────────

function makeChunk(overrides: Record<string, unknown> = {}) {
    return {
        id: overrides.id ?? "chunk-1",
        title: overrides.title ?? "Test Chunk",
        content: overrides.content ?? "Some content here that is reasonably long for testing purposes.",
        type: overrides.type ?? "document",
        rationale: overrides.rationale ?? null,
        alternatives: overrides.alternatives ?? null,
        consequences: overrides.consequences ?? null,
        summary: overrides.summary ?? null,
        embedding: overrides.embedding ?? null,
        reviewStatus: overrides.reviewStatus ?? null,
        updatedAt: overrides.updatedAt ?? new Date(),
        createdAt: overrides.createdAt ?? new Date(),
        userId: "user-1",
        scope: null,
        aliases: null,
        notAbout: null,
        embeddingUpdatedAt: null,
        sourceUrl: null,
        sourceType: null,
    };
}

function makeEnrichedChunk(overrides: Record<string, unknown> = {}): ChunkWithMetadata {
    return {
        id: (overrides.id as string) ?? "chunk-1",
        title: (overrides.title as string) ?? "Test Chunk",
        content: (overrides.content as string) ?? "Some content here that is reasonably long for testing purposes.",
        type: (overrides.type as string) ?? "document",
        rationale: (overrides.rationale as string | null) ?? null,
        tags: (overrides.tags as string[]) ?? [],
        score: (overrides.score as number) ?? 10,
        healthScore: (overrides.healthScore as number) ?? 50,
        isStale: (overrides.isStale as boolean) ?? false,
        hasPendingProposal: (overrides.hasPendingProposal as boolean) ?? false,
    };
}

function setupListChunksMock(chunks: ReturnType<typeof makeChunk>[]) {
    const listChunksMock = listChunks as ReturnType<typeof vi.fn>;
    listChunksMock.mockReturnValue(
        Effect.succeed({ chunks, total: chunks.length })
    );
}

function setupEnrichChunksMock(enriched: ChunkWithMetadata[]) {
    const enrichChunksMock = enrichChunks as ReturnType<typeof vi.fn>;
    enrichChunksMock.mockReturnValue(Effect.succeed(enriched));
}

// ── exportContext tests ─────────────────────────────────────────────

describe("exportContext", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("returns markdown format by default with header", async () => {
        const chunks = [makeChunk({ id: "c1", title: "My Doc", content: "Hello world" })];
        setupListChunksMock(chunks);

        const enriched = [makeEnrichedChunk({ id: "c1", title: "My Doc", content: "Hello world" })];
        setupEnrichChunksMock(enriched);

        const fmtStructuredMock = formatStructured as ReturnType<typeof vi.fn>;
        fmtStructuredMock.mockReturnValue({ sections: [{ title: "Architecture", chunks: enriched }], totalChunks: 1 });

        const fmtMarkdownMock = formatStructuredMarkdown as ReturnType<typeof vi.fn>;
        fmtMarkdownMock.mockReturnValue("# Project Context\n\n## Architecture\n\n### My Doc [health: 50]\n\nHello world");

        const result = await Effect.runPromise(exportContext("user-1", {}));

        expect(result.format).toBe("markdown");
        expect(result.content).toContain("# Project Context");
        expect(result.content).toContain("My Doc");
    });

    it("returns json format when requested", async () => {
        const chunks = [makeChunk({ id: "c1", title: "JSON Chunk" })];
        setupListChunksMock(chunks);

        const enriched = [makeEnrichedChunk({ id: "c1", title: "JSON Chunk" })];
        setupEnrichChunksMock(enriched);

        const result = await Effect.runPromise(exportContext("user-1", { format: "json" }));

        expect(result.format).toBe("json");
        expect(result.chunks).toBeDefined();
        expect(result.chunks![0]!.title).toBe("JSON Chunk");
    });

    it("respects token budget and excludes chunks that exceed it", async () => {
        const longContent = "x".repeat(2000); // ~500 tokens
        const chunks = [
            makeChunk({ id: "c1", title: "First", content: "Short content", type: "document" }),
            makeChunk({ id: "c2", title: "Second", content: longContent, type: "document" }),
            makeChunk({ id: "c3", title: "Third", content: "Also short", type: "document" }),
        ];
        setupListChunksMock(chunks);

        const enriched = [
            makeEnrichedChunk({ id: "c1", title: "First", content: "Short content", score: 10 }),
            makeEnrichedChunk({ id: "c2", title: "Second", content: longContent, score: 10 }),
            makeEnrichedChunk({ id: "c3", title: "Third", content: "Also short", score: 10 }),
        ];
        setupEnrichChunksMock(enriched);

        const fmtStructuredMock = formatStructured as ReturnType<typeof vi.fn>;
        fmtStructuredMock.mockImplementation((budgetedChunks: ChunkWithMetadata[]) => ({
            sections: [{ title: "Architecture", chunks: budgetedChunks }],
            totalChunks: budgetedChunks.length,
        }));

        const fmtMarkdownMock = formatStructuredMarkdown as ReturnType<typeof vi.fn>;
        fmtMarkdownMock.mockImplementation(() => "# Project Context\n\n## Architecture\n\n### First [health: 50]\n\nShort content\n\n### Third [health: 50]\n\nAlso short");

        // Very tight budget — should only fit small chunks
        const result = await Effect.runPromise(exportContext("user-1", { maxTokens: 100 }));

        expect(result.format).toBe("markdown");
        expect(result.tokens).toBeLessThanOrEqual(100);
        // The long chunk should not be included
        expect(result.content).not.toContain(longContent);
    });

    it("produces minimal output for empty chunk list", async () => {
        setupListChunksMock([]);
        setupEnrichChunksMock([]);

        const fmtStructuredMock = formatStructured as ReturnType<typeof vi.fn>;
        fmtStructuredMock.mockReturnValue({ sections: [], totalChunks: 0 });

        const fmtMarkdownMock = formatStructuredMarkdown as ReturnType<typeof vi.fn>;
        fmtMarkdownMock.mockReturnValue("# Project Context\n\n");

        const result = await Effect.runPromise(exportContext("user-1", {}));

        expect(result.format).toBe("markdown");
        expect(result.content).toBe("# Project Context\n\n");
        expect(result.tokens).toBeGreaterThan(0); // header tokens
    });

    it("formats document type as 'Architecture' label", async () => {
        const chunks = [makeChunk({ id: "c1", title: "DB Design", type: "document" })];
        setupListChunksMock(chunks);

        const enriched = [makeEnrichedChunk({ id: "c1", title: "DB Design", type: "document" })];
        setupEnrichChunksMock(enriched);

        const fmtStructuredMock = formatStructured as ReturnType<typeof vi.fn>;
        fmtStructuredMock.mockReturnValue({
            sections: [{ title: "Architecture", chunks: enriched }],
            totalChunks: 1,
        });

        const fmtMarkdownMock = formatStructuredMarkdown as ReturnType<typeof vi.fn>;
        fmtMarkdownMock.mockReturnValue("# Project Context\n\n## Architecture\n\n### DB Design [health: 50]\n\nSome content");

        const result = await Effect.runPromise(exportContext("user-1", { maxTokens: 5000 }));

        expect(result.content).toContain("## Architecture");
    });

    it("includes rationale in formatted output when present", async () => {
        const chunks = [makeChunk({
            id: "c1",
            title: "Decision",
            content: "We chose X.",
            rationale: "Because Y is better than Z.",
        })];
        setupListChunksMock(chunks);

        const enriched = [makeEnrichedChunk({
            id: "c1",
            title: "Decision",
            content: "We chose X.",
            rationale: "Because Y is better than Z.",
        })];
        setupEnrichChunksMock(enriched);

        const fmtStructuredMock = formatStructured as ReturnType<typeof vi.fn>;
        fmtStructuredMock.mockReturnValue({
            sections: [{ title: "Architecture", chunks: enriched }],
            totalChunks: 1,
        });

        const fmtMarkdownMock = formatStructuredMarkdown as ReturnType<typeof vi.fn>;
        fmtMarkdownMock.mockReturnValue("# Project Context\n\n## Architecture\n\n### Decision [health: 50]\n\nWe chose X.\n\n**Rationale:** Because Y is better than Z.");

        const result = await Effect.runPromise(exportContext("user-1", { maxTokens: 5000 }));

        expect(result.content).toContain("**Rationale:** Because Y is better than Z.");
    });

    it("scores document type higher than note type", async () => {
        const chunks = [
            makeChunk({ id: "c1", title: "A Note", type: "note", content: "note content" }),
            makeChunk({ id: "c2", title: "A Doc", type: "document", content: "doc content" }),
        ];
        setupListChunksMock(chunks);

        // enrichChunks returns docs with higher score than notes
        const enriched = [
            makeEnrichedChunk({ id: "c1", title: "A Note", type: "note", content: "note content", score: 5 }),
            makeEnrichedChunk({ id: "c2", title: "A Doc", type: "document", content: "doc content", score: 10 }),
        ];
        setupEnrichChunksMock(enriched);

        const result = await Effect.runPromise(exportContext("user-1", { format: "json", maxTokens: 5000 }));

        // Document should appear before note due to higher score (budgetChunks sorts by score desc)
        expect(result.chunks![0]!.title).toBe("A Doc");
        expect(result.chunks![1]!.title).toBe("A Note");
    });

    it("boosts score for chunks matching forPath context", async () => {
        const otherChunk = makeChunk({ id: "c1", title: "Generic", type: "document", content: "generic doc content that is quite detailed" });
        const fileContextChunk = makeChunk({ id: "c2", title: "Relevant", type: "note", content: "relevant" });
        setupListChunksMock([otherChunk, fileContextChunk]);

        // enrichChunks returns Generic with higher base score, Relevant with lower
        const enriched = [
            makeEnrichedChunk({ id: "c1", title: "Generic", type: "document", content: "generic doc content that is quite detailed", score: 10 }),
            makeEnrichedChunk({ id: "c2", title: "Relevant", type: "note", content: "relevant", score: 5 }),
        ];
        setupEnrichChunksMock(enriched);

        // resolveForFiles returns c2 as file-relevant
        const resolveForFilesMock = resolveForFiles as ReturnType<typeof vi.fn>;
        resolveForFilesMock.mockReturnValue(Effect.succeed(["c2"]));

        const result = await Effect.runPromise(
            exportContext("user-1", { forPath: "src/index.ts", format: "json", maxTokens: 5000 })
        );

        // The file-relevant chunk should be boosted to first position (5 + 15 = 20 > 10)
        expect(result.chunks![0]!.title).toBe("Relevant");
    });

    it("includes tags in json output", async () => {
        const chunks = [makeChunk({ id: "c1", title: "Tagged" })];
        setupListChunksMock(chunks);

        const enriched = [makeEnrichedChunk({
            id: "c1",
            title: "Tagged",
            tags: ["architecture", "backend"],
        })];
        setupEnrichChunksMock(enriched);

        const result = await Effect.runPromise(exportContext("user-1", { format: "json", maxTokens: 5000 }));

        expect(result.chunks![0]!.tags).toEqual(["architecture", "backend"]);
    });

    it("approved chunks are scored higher via reviewStatus", async () => {
        const chunks = [
            makeChunk({ id: "c1", title: "Unapproved", type: "note", reviewStatus: null }),
            makeChunk({ id: "c2", title: "Approved", type: "note", reviewStatus: "approved" }),
        ];
        setupListChunksMock(chunks);

        // enrichChunks returns approved with higher score
        const enriched = [
            makeEnrichedChunk({ id: "c1", title: "Unapproved", type: "note", score: 5 }),
            makeEnrichedChunk({ id: "c2", title: "Approved", type: "note", score: 10 }),
        ];
        setupEnrichChunksMock(enriched);

        const result = await Effect.runPromise(exportContext("user-1", { format: "json", maxTokens: 5000 }));

        expect(result.chunks![0]!.title).toBe("Approved");
    });
});

// ── generateClaudeMd tests ──────────────────────────────────────────

describe("generateClaudeMd", () => {
    it("returns empty message when no chunks found", async () => {
        const mock = listChunksByTag as ReturnType<typeof vi.fn>;
        mock.mockReturnValue(Effect.succeed([]));

        const result = await Effect.runPromise(generateClaudeMd({ userId: "user-1" }));

        expect(result.chunks).toBe(0);
        expect(result.content).toContain("# Project Context");
        expect(result.content).toContain('No chunks found with tag "claude-context"');
    });

    it("uses custom tag name when provided", async () => {
        const mock = listChunksByTag as ReturnType<typeof vi.fn>;
        mock.mockReturnValue(Effect.succeed([]));

        const result = await Effect.runPromise(generateClaudeMd({ userId: "user-1", tag: "my-tag" }));

        expect(result.content).toContain('No chunks found with tag "my-tag"');
    });

    it("groups chunks by type into sections", async () => {
        const mock = listChunksByTag as ReturnType<typeof vi.fn>;
        mock.mockReturnValue(Effect.succeed([
            { id: "1", title: "Naming Convention", content: "Use camelCase", type: "note", rationale: null, summary: null },
            { id: "2", title: "System Architecture", content: "Microservices", type: "document", rationale: null, summary: null },
            { id: "3", title: "API Docs", content: "REST API spec", type: "reference", rationale: null, summary: null },
        ]));

        const result = await Effect.runPromise(generateClaudeMd({ userId: "user-1" }));

        expect(result.chunks).toBe(3);
        expect(result.content).toContain("## Conventions");
        expect(result.content).toContain("## Architecture");
        expect(result.content).toContain("## References");
        expect(result.content).toContain("### Naming Convention");
        expect(result.content).toContain("### System Architecture");
        expect(result.content).toContain("### API Docs");
    });

    it("outputs sections in correct order: Conventions, Architecture, References, Other", async () => {
        const mock = listChunksByTag as ReturnType<typeof vi.fn>;
        mock.mockReturnValue(Effect.succeed([
            { id: "1", title: "Ref", content: "ref", type: "reference", rationale: null, summary: null },
            { id: "2", title: "Conv", content: "conv", type: "note", rationale: null, summary: null },
            { id: "3", title: "Arch", content: "arch", type: "document", rationale: null, summary: null },
            { id: "4", title: "Other", content: "other", type: "checklist", rationale: null, summary: null },
        ]));

        const result = await Effect.runPromise(generateClaudeMd({ userId: "user-1" }));

        const convIdx = result.content.indexOf("## Conventions");
        const archIdx = result.content.indexOf("## Architecture");
        const refIdx = result.content.indexOf("## References");
        const otherIdx = result.content.indexOf("## Other");

        expect(convIdx).toBeLessThan(archIdx);
        expect(archIdx).toBeLessThan(refIdx);
        expect(refIdx).toBeLessThan(otherIdx);
    });

    it("includes rationale in chunk entries when present", async () => {
        const mock = listChunksByTag as ReturnType<typeof vi.fn>;
        mock.mockReturnValue(Effect.succeed([
            { id: "1", title: "Decision", content: "We chose X", type: "document", rationale: "Speed matters", summary: null },
        ]));

        const result = await Effect.runPromise(generateClaudeMd({ userId: "user-1" }));

        expect(result.content).toContain("**Rationale:** Speed matters");
    });

    it("produces valid markdown structure", async () => {
        const mock = listChunksByTag as ReturnType<typeof vi.fn>;
        mock.mockReturnValue(Effect.succeed([
            { id: "1", title: "Test", content: "Content", type: "note", rationale: null, summary: null },
        ]));

        const result = await Effect.runPromise(generateClaudeMd({ userId: "user-1" }));

        // Starts with H1
        expect(result.content).toMatch(/^# Project Context/);
        // Contains H2 section
        expect(result.content).toContain("## Conventions");
        // Contains H3 entry
        expect(result.content).toContain("### Test");
    });
});
