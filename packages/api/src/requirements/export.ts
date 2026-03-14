import type { RequirementStep } from "@fubbik/db/schema/requirement";

function interpolate(text: string, params?: Record<string, string>): string {
    if (!params) return text;
    return text.replace(/\{(\w+)\}/g, (_, key) => params[key] ?? `{${key}}`);
}

function capitalize(keyword: string): string {
    return keyword.charAt(0).toUpperCase() + keyword.slice(1);
}

export function toGherkin(title: string, steps: RequirementStep[]): string {
    const lines = [`Feature: ${title}`, "", `  Scenario: ${title}`];
    for (const step of steps) {
        const keyword = capitalize(step.keyword);
        const text = interpolate(step.text, step.params);
        lines.push(`    ${keyword} ${text}`);
    }
    return lines.join("\n");
}

export function toVitest(title: string, steps: RequirementStep[]): string {
    const stepComments = steps
        .map(s => {
            const keyword = capitalize(s.keyword);
            const text = interpolate(s.text, s.params);
            return `    // ${keyword} ${text}`;
        })
        .join("\n");

    return [
        `describe("${title}", () => {`,
        `  it("${title}", () => {`,
        stepComments,
        `    throw new Error("Not implemented");`,
        "  });",
        "});"
    ].join("\n");
}

export function toMarkdown(title: string, steps: RequirementStep[]): string {
    const lines = [`# ${title}`, ""];
    for (const step of steps) {
        const keyword = capitalize(step.keyword);
        const text = interpolate(step.text, step.params);
        lines.push(`- [ ] **${keyword}** ${text}`);
    }
    return lines.join("\n");
}
