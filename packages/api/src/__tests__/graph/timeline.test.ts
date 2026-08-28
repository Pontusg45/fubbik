import { describe, expect, it } from "vitest";

import { reconstructGraphAt } from "../../graph/timeline-service";

describe("timeline service", () => {
    it("exports reconstructGraphAt", () => {
        expect(typeof reconstructGraphAt).toBe("function");
    });
});
