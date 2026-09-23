import { describe, expect, it } from "vitest";

import { buildChunkIndex, buildVocabularyIndex, buildFileRefIndex, matchInCode, matchVocabularyInText } from "./smart-link-provider";

describe("buildChunkIndex", () => {
    it("indexes by lowercase title", () => {
        // Given the inline inputs and test fixtures.
        // When
        const index = buildChunkIndex([
            { id: "c1", title: "UserService", aliases: [] },
            { id: "c2", title: "Auth Flow", aliases: ["authentication"] }
        ]);
        // Then
        expect(index.get("userservice")).toEqual({ id: "c1", title: "UserService" });
        expect(index.get("auth flow")).toEqual({ id: "c2", title: "Auth Flow" });
    });

    it("indexes aliases", () => {
        // Given the inline inputs and test fixtures.
        // When
        const index = buildChunkIndex([{ id: "c1", title: "UserService", aliases: ["UserSvc", "user-service"] }]);
        // Then
        expect(index.get("usersvc")).toEqual({ id: "c1", title: "UserService" });
        expect(index.get("user-service")).toEqual({ id: "c1", title: "UserService" });
    });

    it("skips short titles (< 4 chars)", () => {
        // Given the inline inputs and test fixtures.
        // When
        const index = buildChunkIndex([{ id: "c1", title: "API", aliases: [] }]);
        // Then
        expect(index.get("api")).toBeUndefined();
    });

    it("excludes specified chunk id", () => {
        // Given
        const index = buildChunkIndex([
            { id: "c1", title: "UserService", aliases: [] },
            { id: "c2", title: "Auth Flow", aliases: [] }
        ]);
        // When
        const match = matchInCode("UserService", index, new Map(), new Map(), "c1");
        // Then
        expect(match).toBeNull();
    });
});

describe("buildVocabularyIndex", () => {
    it("indexes by lowercase word", () => {
        // Given the inline inputs and test fixtures.
        // When
        const index = buildVocabularyIndex([{ word: "UserService", category: "actor", expects: ["class"] }]);
        // Then
        expect(index.get("userservice")).toEqual({
            word: "UserService",
            category: "actor",
            definition: null,
            expects: ["class"]
        });
    });
});

describe("buildFileRefIndex", () => {
    it("indexes by filename and full path", () => {
        // Given the inline inputs and test fixtures.
        // When
        const index = buildFileRefIndex([{ chunkId: "c1", chunkTitle: "Auth Module", path: "src/auth/service.ts", anchor: null }]);
        // Then
        expect(index.get("src/auth/service.ts")).toEqual({ chunkId: "c1", chunkTitle: "Auth Module", path: "src/auth/service.ts" });
        expect(index.get("service.ts")).toEqual({ chunkId: "c1", chunkTitle: "Auth Module", path: "src/auth/service.ts" });
    });
});

describe("matchInCode", () => {
    const chunks = buildChunkIndex([
        { id: "c1", title: "UserService", aliases: [] },
        { id: "c2", title: "AuthFlow", aliases: [] }
    ]);
    const vocab = buildVocabularyIndex([{ word: "Repository", category: "actor", expects: ["class"] }]);
    const fileRefs = buildFileRefIndex([{ chunkId: "c3", chunkTitle: "Config", path: "src/config.ts", anchor: null }]);

    it("returns chunk match (highest priority)", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = matchInCode("UserService", chunks, fileRefs, vocab);
        // Then
        expect(result).toEqual({ type: "chunk", id: "c1", title: "UserService" });
    });

    it("returns file ref match when no chunk matches", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = matchInCode("src/config.ts", chunks, fileRefs, vocab);
        // Then
        expect(result).toEqual({ type: "fileRef", chunkId: "c3", chunkTitle: "Config", path: "src/config.ts" });
    });

    it("returns vocabulary match as fallback", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = matchInCode("Repository", chunks, fileRefs, vocab);
        // Then
        expect(result).toEqual({ type: "vocabulary", word: "Repository", category: "actor", definition: null, expects: ["class"] });
    });

    it("returns null when nothing matches", () => {
        // Given the inline inputs and test fixtures.
        // When
        const result = matchInCode("UnknownThing", chunks, fileRefs, vocab);
        // Then
        expect(result).toBeNull();
    });
});

describe("matchVocabularyInText", () => {
    const vocab = buildVocabularyIndex([
        { word: "UserService", category: "actor", expects: ["class"] },
        { word: "deploy", category: "action", expects: null }
    ]);

    it("finds vocabulary terms in plain text", () => {
        // Given the inline inputs and test fixtures.
        // When
        const matches = matchVocabularyInText("The UserService handles deploy requests", vocab);
        // Then
        expect(matches).toHaveLength(2);
        expect(matches[0]).toEqual({ start: 4, end: 15, word: "UserService", category: "actor", definition: null, expects: ["class"] });
        expect(matches[1]).toEqual({ start: 24, end: 30, word: "deploy", category: "action", definition: null, expects: null });
    });

    it("returns empty for no matches", () => {
        // Given the inline inputs and test fixtures.
        // When
        const matches = matchVocabularyInText("nothing relevant here", vocab);
        // Then
        expect(matches).toHaveLength(0);
    });

    it("matches case-insensitively", () => {
        // Given the inline inputs and test fixtures.
        // When
        const matches = matchVocabularyInText("the userservice is important", vocab);
        // Then
        expect(matches).toHaveLength(1);
        expect(matches[0]?.word).toBe("UserService");
    });
});
