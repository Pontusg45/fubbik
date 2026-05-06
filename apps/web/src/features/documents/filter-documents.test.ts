import { describe, expect, it } from "vitest";
import { filterDocuments, groupDocuments, type EnrichedDocument } from "./filter-documents";

const docs: EnrichedDocument[] = [
    { id: "1", title: "Auth Guide", sourcePath: "docs/guides/auth.md", tags: ["auth", "guides"], type: "document", chunkCount: 3 },
    { id: "2", title: "API Endpoints", sourcePath: "docs/api/endpoints.md", tags: ["api", "reference"], type: "reference", chunkCount: 5 },
    { id: "3", title: "Architecture", sourcePath: "docs/architecture.md", tags: ["backend"], type: "document", chunkCount: 4 },
    { id: "4", title: "Errors", sourcePath: "docs/api/errors.md", tags: [], type: "document", chunkCount: 2 },
];

describe("filterDocuments", () => {
    it("returns all documents when no filters active", () => {
        const result = filterDocuments(docs, { activeTags: [], activeTypes: [] });
        expect(result).toHaveLength(4);
    });

    it("filters by tags (OR within tags)", () => {
        const result = filterDocuments(docs, { activeTags: ["auth", "api"], activeTypes: [] });
        expect(result.map(d => d.id)).toEqual(["1", "2"]);
    });

    it("filters by types (OR within types)", () => {
        const result = filterDocuments(docs, { activeTags: [], activeTypes: ["reference"] });
        expect(result.map(d => d.id)).toEqual(["2"]);
    });

    it("AND between dimensions", () => {
        const result = filterDocuments(docs, { activeTags: ["auth", "api"], activeTypes: ["document"] });
        expect(result.map(d => d.id)).toEqual(["1"]);
    });

    it("empty result when no match", () => {
        const result = filterDocuments(docs, { activeTags: ["nonexistent"], activeTypes: [] });
        expect(result).toHaveLength(0);
    });
});

describe("groupDocuments", () => {
    it("groups by folder", () => {
        const groups = groupDocuments(docs, "folder");
        expect(groups.get("docs/guides")).toHaveLength(1);
        expect(groups.get("docs/api")).toHaveLength(2);
        expect(groups.get("docs")).toHaveLength(1);
    });

    it("groups by tag", () => {
        const groups = groupDocuments(docs, "tag");
        expect(groups.get("auth")).toHaveLength(1);
        expect(groups.get("api")).toHaveLength(1);
        expect(groups.get("reference")).toHaveLength(1);
        expect(groups.get("backend")).toHaveLength(1);
        expect(groups.get("guides")).toHaveLength(1);
        expect(groups.get("Untagged")).toHaveLength(1);
    });

    it("duplicates multi-tagged docs across tag groups", () => {
        const groups = groupDocuments([docs[0]!], "tag");
        expect(groups.get("auth")).toHaveLength(1);
        expect(groups.get("guides")).toHaveLength(1);
    });
});
