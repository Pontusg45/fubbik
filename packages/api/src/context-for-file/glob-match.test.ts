import { describe, expect, it } from "vitest";
import { globMatch, normalizePath } from "./glob-match";

describe("normalizePath", () => {
    it("strips leading ./", () => {
        expect(normalizePath("./src/auth/service.ts")).toBe("src/auth/service.ts");
    });

    it("strips leading /", () => {
        expect(normalizePath("/src/auth/service.ts")).toBe("src/auth/service.ts");
    });

    it("collapses consecutive slashes", () => {
        expect(normalizePath("src//auth///service.ts")).toBe("src/auth/service.ts");
    });

    it("strips trailing /", () => {
        expect(normalizePath("src/auth/")).toBe("src/auth");
    });

    it("handles combined edge cases", () => {
        expect(normalizePath("./src//auth/./service.ts")).toBe("src/auth/./service.ts");
    });

    it("returns empty string unchanged", () => {
        expect(normalizePath("")).toBe("");
    });

    it("handles already-clean paths", () => {
        expect(normalizePath("src/auth/service.ts")).toBe("src/auth/service.ts");
    });
});

describe("globMatch with normalization", () => {
    it("matches ./src/x.ts against src/**/*.ts pattern", () => {
        expect(globMatch("src/**/*.ts", "./src/auth/service.ts")).toBe(true);
    });

    it("matches /src/x.ts against src/**/*.ts pattern", () => {
        expect(globMatch("src/**/*.ts", "/src/auth/service.ts")).toBe(true);
    });

    it("normalizes the pattern too", () => {
        expect(globMatch("./src/**/*.ts", "src/auth/service.ts")).toBe(true);
    });
});
