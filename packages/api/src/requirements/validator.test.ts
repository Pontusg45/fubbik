import type { RequirementStep } from "@fubbik/db/schema/requirement";
import { describe, expect, it } from "vitest";

import { validateSteps } from "./validator";

function step(keyword: RequirementStep["keyword"], text = "something"): RequirementStep {
    return { keyword, text };
}

describe("validateSteps", () => {
    it("accepts a valid given-when-then sequence", () => {
        const errors = validateSteps([step("given"), step("when"), step("then")]);
        expect(errors).toEqual([]);
    });

    it("accepts and/but continuations", () => {
        const errors = validateSteps([
            step("given"),
            step("and"),
            step("when"),
            step("but"),
            step("then"),
            step("and")
        ]);
        expect(errors).toEqual([]);
    });

    it("rejects empty steps", () => {
        const errors = validateSteps([]);
        expect(errors).toHaveLength(1);
        expect(errors[0].error).toMatch(/at least one step/);
    });

    it("rejects wrong first keyword", () => {
        const errors = validateSteps([step("when"), step("then")]);
        expect(errors.some(e => e.step === 0 && e.error.includes("First step"))).toBe(true);
    });

    it("rejects and/but as first step", () => {
        const errors = validateSteps([step("and"), step("when"), step("then")]);
        expect(errors.some(e => e.step === 0)).toBe(true);
    });

    it("rejects missing when", () => {
        const errors = validateSteps([step("given"), step("then")]);
        expect(errors.some(e => e.error.includes("'when'"))).toBe(true);
    });

    it("rejects missing then", () => {
        const errors = validateSteps([step("given"), step("when")]);
        expect(errors.some(e => e.error.includes("'then'"))).toBe(true);
    });

    it("rejects given after when", () => {
        const errors = validateSteps([step("given"), step("when"), step("given"), step("then")]);
        expect(errors.some(e => e.step === 2 && e.error.includes("given"))).toBe(true);
    });

    it("rejects given after then", () => {
        const errors = validateSteps([step("given"), step("when"), step("then"), step("given")]);
        expect(errors.some(e => e.step === 3 && e.error.includes("given"))).toBe(true);
    });

    it("rejects then before when phase", () => {
        const errors = validateSteps([step("given"), step("then"), step("when")]);
        expect(errors.some(e => e.error.includes("'then' must come after"))).toBe(true);
    });
});
