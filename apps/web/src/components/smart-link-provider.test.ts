import { describe, expect, it } from "vitest";
import { buildChunkIndex, buildVocabularyIndex, buildFileRefIndex, matchInCode, matchVocabularyInText } from "./smart-link-provider";

describe("buildChunkIndex", () => {
    it("indexes by lowercase title", () => {
        const index = buildChunkIndex([
            { id: "c1", title: "UserService", aliases: [] },
            { id: "c2", title: "Auth Flow", aliases: ["authentication"] }
        ]);
        expect(index.get("userservice")).toEqual({ id: "c1", title: "UserService" });
        expect(index.get("auth flow")).toEqual({ id: "c2", title: "Auth Flow" });
    });

    it("indexes aliases", () => {
        const index = buildChunkIndex([
            { id: "c1", title: "UserService", aliases: ["UserSvc", "user-service"] }
        ]);
        expect(index.get("usersvc")).toEqual({ id: "c1", title: "UserService" });
        expect(index.get("user-service")).toEqual({ id: "c1", title: "UserService" });
    });

    it("skips short titles (< 4 chars)", () => {
        const index = buildChunkIndex([
            { id: "c1", title: "API", aliases: [] }
        ]);
        expect(index.get("api")).toBeUndefined();
    });

    it("excludes specified chunk id", () => {
        const index = buildChunkIndex([
            { id: "c1", title: "UserService", aliases: [] },
            { id: "c2", title: "Auth Flow", aliases: [] }
        ]);
        const match = matchInCode("UserService", index, new Map(), new Map(), "c1");
        expect(match).toBeNull();
    });
});

describe("buildVocabularyIndex", () => {
    it("indexes by lowercase word", () => {
        const index = buildVocabularyIndex([
            { word: "UserService", category: "actor", expects: ["class"] }
        ]);
        expect(index.get("userservice")).toEqual({
            word: "UserService",
            category: "actor",
            expects: ["class"]
        });
    });
});

describe("buildFileRefIndex", () => {
    it("indexes by filename and full path", () => {
        const index = buildFileRefIndex([
            { chunkId: "c1", chunkTitle: "Auth Module", path: "src/auth/service.ts", anchor: null }
        ]);
        expect(index.get("src/auth/service.ts")).toEqual({ chunkId: "c1", chunkTitle: "Auth Module", path: "src/auth/service.ts" });
        expect(index.get("service.ts")).toEqual({ chunkId: "c1", chunkTitle: "Auth Module", path: "src/auth/service.ts" });
    });
});

describe("matchInCode", () => {
    const chunks = buildChunkIndex([
        { id: "c1", title: "UserService", aliases: [] },
        { id: "c2", title: "AuthFlow", aliases: [] }
    ]);
    const vocab = buildVocabularyIndex([
        { word: "Repository", category: "actor", expects: ["class"] }
    ]);
    const fileRefs = buildFileRefIndex([
        { chunkId: "c3", chunkTitle: "Config", path: "src/config.ts", anchor: null }
    ]);

    it("returns chunk match (highest priority)", () => {
        const result = matchInCode("UserService", chunks, fileRefs, vocab);
        expect(result).toEqual({ type: "chunk", id: "c1", title: "UserService" });
    });

    it("returns file ref match when no chunk matches", () => {
        const result = matchInCode("src/config.ts", chunks, fileRefs, vocab);
        expect(result).toEqual({ type: "fileRef", chunkId: "c3", chunkTitle: "Config", path: "src/config.ts" });
    });

    it("returns vocabulary match as fallback", () => {
        const result = matchInCode("Repository", chunks, fileRefs, vocab);
        expect(result).toEqual({ type: "vocabulary", word: "Repository", category: "actor", expects: ["class"] });
    });

    it("returns null when nothing matches", () => {
        const result = matchInCode("UnknownThing", chunks, fileRefs, vocab);
        expect(result).toBeNull();
    });
});

describe("matchVocabularyInText", () => {
    const vocab = buildVocabularyIndex([
        { word: "UserService", category: "actor", expects: ["class"] },
        { word: "deploy", category: "action", expects: null }
    ]);

    it("finds vocabulary terms in plain text", () => {
        const matches = matchVocabularyInText("The UserService handles deploy requests", vocab);
        expect(matches).toHaveLength(2);
        expect(matches[0]).toEqual({ start: 4, end: 15, word: "UserService", category: "actor", expects: ["class"] });
        expect(matches[1]).toEqual({ start: 24, end: 30, word: "deploy", category: "action", expects: null });
    });

    it("returns empty for no matches", () => {
        const matches = matchVocabularyInText("nothing relevant here", vocab);
        expect(matches).toHaveLength(0);
    });

    it("matches case-insensitively", () => {
        const matches = matchVocabularyInText("the userservice is important", vocab);
        expect(matches).toHaveLength(1);
        expect(matches[0]?.word).toBe("UserService");
    });
});
