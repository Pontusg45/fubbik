import { describe, expect, it } from "vitest";

import { indexDirectory, indexFile } from "../code-index/service";

describe("code-index service", () => {
    it("exports indexFile", () => {
        expect(typeof indexFile).toBe("function");
    });

    it("exports indexDirectory", () => {
        expect(typeof indexDirectory).toBe("function");
    });
});
