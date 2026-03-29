import { describe, it, expect } from "vitest";
import { generateStepsFromRequirements } from "./generate-from-requirements";

const makeRequirement = (
    overrides: Partial<{
        id: string;
        title: string;
        description: string | null;
        steps: Array<{ keyword: string; text: string }>;
        status: string;
        priority: string | null;
    }> = {}
) => ({
    id: "req-1",
    title: "User can log in",
    description: "Basic login flow",
    steps: [],
    status: "untested",
    priority: "must",
    ...overrides
});

describe("generateStepsFromRequirements", () => {
    describe("standard template", () => {
        it("generates Implement and Verify steps for a requirement", () => {
            const reqs = [makeRequirement()];
            const steps = generateStepsFromRequirements(reqs);

            expect(steps).toHaveLength(2);
            expect(steps[0]!.description).toBe("Implement: User can log in");
            expect(steps[1]!.description).toBe("Verify: User can log in");
        });

        it("includes BDD sub-steps between Implement and Verify", () => {
            const reqs = [
                makeRequirement({
                    steps: [
                        { keyword: "given", text: "a registered user" },
                        { keyword: "when", text: "they enter credentials" },
                        { keyword: "then", text: "they see the dashboard" }
                    ]
                })
            ];
            const steps = generateStepsFromRequirements(reqs);

            expect(steps).toHaveLength(5);
            expect(steps[0]!.description).toBe("Implement: User can log in");
            expect(steps[1]!.description).toBe("  Given a registered user");
            expect(steps[2]!.description).toBe("  When they enter credentials");
            expect(steps[3]!.description).toBe("  Then they see the dashboard");
            expect(steps[4]!.description).toBe("Verify: User can log in");
        });

        it("capitalizes BDD keyword first letter", () => {
            const reqs = [
                makeRequirement({
                    steps: [{ keyword: "and", text: "something else" }]
                })
            ];
            const steps = generateStepsFromRequirements(reqs);

            expect(steps[1]!.description).toBe("  And something else");
        });

        it("assigns sequential order numbers across requirements", () => {
            const reqs = [
                makeRequirement({ id: "req-1", title: "First" }),
                makeRequirement({ id: "req-2", title: "Second" })
            ];
            const steps = generateStepsFromRequirements(reqs);

            expect(steps.map((s) => s.order)).toEqual([0, 1, 2, 3]);
        });

        it("links each step to its requirement via requirementId", () => {
            const reqs = [
                makeRequirement({ id: "req-1", title: "First" }),
                makeRequirement({ id: "req-2", title: "Second" })
            ];
            const steps = generateStepsFromRequirements(reqs);

            expect(steps[0]!.requirementId).toBe("req-1");
            expect(steps[1]!.requirementId).toBe("req-1");
            expect(steps[2]!.requirementId).toBe("req-2");
            expect(steps[3]!.requirementId).toBe("req-2");
        });
    });

    describe("detailed template", () => {
        it("generates four steps per requirement: Verify, Implement, Test, Document", () => {
            const reqs = [makeRequirement()];
            const steps = generateStepsFromRequirements(reqs, "detailed");

            expect(steps).toHaveLength(4);
            expect(steps[0]!.description).toBe(
                "Verify requirements: User can log in"
            );
            expect(steps[1]!.description).toBe("Implement: User can log in");
            expect(steps[2]!.description).toBe("Test: User can log in");
            expect(steps[3]!.description).toBe("Document: User can log in");
        });

        it("assigns sequential order numbers", () => {
            const reqs = [
                makeRequirement({ id: "req-1", title: "A" }),
                makeRequirement({ id: "req-2", title: "B" })
            ];
            const steps = generateStepsFromRequirements(reqs, "detailed");

            expect(steps).toHaveLength(8);
            expect(steps.map((s) => s.order)).toEqual([
                0, 1, 2, 3, 4, 5, 6, 7
            ]);
        });

        it("does not include BDD sub-steps even if requirement has them", () => {
            const reqs = [
                makeRequirement({
                    steps: [
                        { keyword: "given", text: "something" },
                        { keyword: "then", text: "result" }
                    ]
                })
            ];
            const steps = generateStepsFromRequirements(reqs, "detailed");

            // Detailed template always produces exactly 4 steps per requirement
            expect(steps).toHaveLength(4);
        });
    });

    describe("edge cases", () => {
        it("returns empty array for empty requirements list", () => {
            const steps = generateStepsFromRequirements([]);
            expect(steps).toEqual([]);
        });

        it("defaults to standard template when no template specified", () => {
            const reqs = [makeRequirement()];
            const steps = generateStepsFromRequirements(reqs);

            // Standard produces 2 steps (Implement + Verify) for a requirement with no BDD steps
            expect(steps).toHaveLength(2);
            expect(steps[0]!.description).toContain("Implement:");
            expect(steps[1]!.description).toContain("Verify:");
        });

        it("handles requirement with empty steps array in standard mode", () => {
            const reqs = [makeRequirement({ steps: [] })];
            const steps = generateStepsFromRequirements(reqs);

            expect(steps).toHaveLength(2);
        });

        it("handles multiple requirements with mixed BDD steps", () => {
            const reqs = [
                makeRequirement({
                    id: "req-1",
                    title: "With steps",
                    steps: [{ keyword: "given", text: "a thing" }]
                }),
                makeRequirement({
                    id: "req-2",
                    title: "Without steps",
                    steps: []
                })
            ];
            const steps = generateStepsFromRequirements(reqs);

            // req-1: Implement + 1 BDD + Verify = 3
            // req-2: Implement + Verify = 2
            expect(steps).toHaveLength(5);
            expect(steps.map((s) => s.order)).toEqual([0, 1, 2, 3, 4]);
        });
    });
});
