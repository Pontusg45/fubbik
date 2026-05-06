import { describe, expect, it } from "vitest";
import { splitMarkdown } from "./split-markdown";
import { renderMarkdown } from "./render-markdown";

describe("renderDocument round-trip format", () => {
    it("round-trips frontmatter through split → render → split", () => {
        const md = [
            "---",
            "title: Auth Guide",
            "type: reference",
            "tags:",
            "  - security",
            "  - backend",
            "---",
            "",
            "## Setup",
            "",
            "Install the auth library.",
            "",
            "## Configuration",
            "",
            "Edit config.json.",
        ].join("\n");

        const first = splitMarkdown(md, "docs/auth.md");
        expect(first.title).toBe("Auth Guide");
        expect(first.sections).toHaveLength(2);

        const rendered = renderMarkdown({
            title: first.title,
            type: "reference",
            tags: ["security", "backend"],
            splitLevel: first.splitLevel,
            sections: first.sections,
        });

        const second = splitMarkdown(rendered, "docs/auth.md");
        expect(second.title).toBe(first.title);
        expect(second.sections).toHaveLength(first.sections.length);
        for (let i = 0; i < first.sections.length; i++) {
            expect(second.sections[i]!.title).toBe(first.sections[i]!.title);
            expect(second.sections[i]!.content).toBe(first.sections[i]!.content);
        }
    });

    it("round-trips decision context", () => {
        const md = [
            "---",
            "title: Decisions",
            "---",
            "",
            "## Token Strategy",
            "",
            "We use JWT.",
            "",
            "> **Rationale:** Stateless auth.",
            "",
            "> **Alternatives:**",
            "> - Sessions",
            "> - OAuth",
            "",
            "> **Consequences:** Need refresh tokens.",
        ].join("\n");

        const first = splitMarkdown(md, "test.md");
        expect(first.sections[0]!.rationale).toBe("Stateless auth.");

        const rendered = renderMarkdown({
            title: first.title,
            tags: [],
            splitLevel: first.splitLevel,
            sections: first.sections,
        });

        const second = splitMarkdown(rendered, "test.md");
        expect(second.sections[0]!.rationale).toBe(first.sections[0]!.rationale);
        expect(second.sections[0]!.alternatives).toEqual(first.sections[0]!.alternatives);
        expect(second.sections[0]!.consequences).toBe(first.sections[0]!.consequences);
        expect(second.sections[0]!.content).toBe(first.sections[0]!.content);
    });
});
