import { describe, it, expect } from "vitest";
import { parsePlanMarkdown } from "./parse-plan-markdown";

describe("parsePlanMarkdown", () => {
    it("parses title from first H1", () => {
        const md = "# My Feature Plan\n\n**Goal:** Build something\n";
        const result = parsePlanMarkdown(md);
        expect(result.title).toBe("My Feature Plan");
    });

    it("parses goal as description", () => {
        const md = "# Title\n\n**Goal:** Build the thing\n";
        const result = parsePlanMarkdown(md);
        expect(result.description).toBe("Build the thing");
    });

    it("extracts steps from checkboxes", () => {
        const md = `# Plan
**Goal:** Do stuff

## Task 1: Schema

- [ ] **Step 1: Create the table**
- [ ] **Step 2: Push schema**

## Task 2: API

- [ ] **Step 1: Add endpoint**
- [x] **Step 2: Test it**
`;
        const result = parsePlanMarkdown(md);
        expect(result.steps).toHaveLength(4);
        expect(result.steps[0]!.description).toBe("Create the table");
        expect(result.steps[0]!.taskGroup).toBe("Task 1: Schema");
        expect(result.steps[2]!.description).toBe("Add endpoint");
        expect(result.steps[2]!.taskGroup).toBe("Task 2: API");
    });

    it("handles steps without Step N prefix", () => {
        const md =
            "# Plan\n\n- [ ] **Do the thing**\n- [ ] **Do another thing**\n";
        const result = parsePlanMarkdown(md);
        expect(result.steps).toHaveLength(2);
        expect(result.steps[0]!.description).toBe("Do the thing");
    });

    it("defaults title when no H1 found", () => {
        const md = "Some content without a heading\n";
        const result = parsePlanMarkdown(md);
        expect(result.title).toBe("Imported Plan");
    });

    it("defaults description when no Goal found", () => {
        const md = "# Title\n\nNo goal here\n";
        const result = parsePlanMarkdown(md);
        expect(result.description).toBe("");
    });

    it("assigns sequential order numbers", () => {
        const md = `# Plan
- [ ] **First**
- [ ] **Second**
- [ ] **Third**
`;
        const result = parsePlanMarkdown(md);
        expect(result.steps.map((s) => s.order)).toEqual([0, 1, 2]);
    });

    it("does not assign taskGroup when no task header precedes steps", () => {
        const md = "# Plan\n\n- [ ] **Standalone step**\n";
        const result = parsePlanMarkdown(md);
        expect(result.steps[0]!.taskGroup).toBeUndefined();
    });
});
