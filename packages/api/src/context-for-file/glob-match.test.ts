import { describe, expect, it } from "vitest";

import { globMatch, normalizePath } from "./glob-match";

describe("normalizePath", () => {
    it("strips leading ./", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizePath("./src/auth/service.ts")).toBe("src/auth/service.ts");
    });

    it("strips leading /", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizePath("/src/auth/service.ts")).toBe("src/auth/service.ts");
    });

    it("collapses consecutive slashes", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizePath("src//auth///service.ts")).toBe("src/auth/service.ts");
    });

    it("strips trailing /", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizePath("src/auth/")).toBe("src/auth");
    });

    it("handles combined edge cases", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizePath("./src//auth/./service.ts")).toBe("src/auth/./service.ts");
    });

    it("returns empty string unchanged", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizePath("")).toBe("");
    });

    it("handles already-clean paths", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizePath("src/auth/service.ts")).toBe("src/auth/service.ts");
    });
});

describe("globMatch with normalization", () => {
    it("matches ./src/x.ts against src/**/*.ts pattern", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(globMatch("src/**/*.ts", "./src/auth/service.ts")).toBe(true);
    });

    it("matches /src/x.ts against src/**/*.ts pattern", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(globMatch("src/**/*.ts", "/src/auth/service.ts")).toBe(true);
    });

    it("normalizes the pattern too", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(globMatch("./src/**/*.ts", "src/auth/service.ts")).toBe(true);
    });
});
