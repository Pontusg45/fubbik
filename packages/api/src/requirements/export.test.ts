import type { RequirementStep } from "@fubbik/db/schema/requirement";
import { describe, expect, it } from "vitest";

import { toGherkin, toMarkdown, toVitest } from "./export";

const steps: RequirementStep[] = [
    { keyword: "given", text: "a user named {name}", params: { name: "Alice" } },
    { keyword: "when", text: "they log in" },
    { keyword: "then", text: "they see the dashboard" }
];

describe("toGherkin", () => {
    it("produces valid Gherkin output", () => {
        const result = toGherkin("User Login", steps);
        expect(result).toContain("Feature: User Login");
        expect(result).toContain("Scenario: User Login");
        expect(result).toContain("Given a user named Alice");
        expect(result).toContain("When they log in");
        expect(result).toContain("Then they see the dashboard");
    });

    it("preserves unmatched params placeholders", () => {
        const result = toGherkin("Test", [{ keyword: "given", text: "a {missing} value" }]);
        expect(result).toContain("Given a {missing} value");
    });
});

describe("toVitest", () => {
    it("produces a describe/it block with step comments", () => {
        const result = toVitest("User Login", steps);
        expect(result).toContain('describe("User Login"');
        expect(result).toContain('it("User Login"');
        expect(result).toContain("// Given a user named Alice");
        expect(result).toContain("// When they log in");
        expect(result).toContain("// Then they see the dashboard");
        expect(result).toContain('throw new Error("Not implemented")');
    });
});

describe("toMarkdown", () => {
    it("produces a heading with checkbox list", () => {
        const result = toMarkdown("User Login", steps);
        expect(result).toContain("# User Login");
        expect(result).toContain("- [ ] **Given** a user named Alice");
        expect(result).toContain("- [ ] **When** they log in");
        expect(result).toContain("- [ ] **Then** they see the dashboard");
    });
});
