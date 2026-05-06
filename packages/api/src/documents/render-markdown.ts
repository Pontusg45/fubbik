import type { MarkdownSection } from "./split-markdown";
import { tagsFromPath } from "../chunks/parse-docs";

interface RenderOptions {
    title: string;
    type?: string;
    tags: string[];
    scope?: Record<string, string>;
    splitLevel: number;
    sections: MarkdownSection[];
    sourcePath?: string;
}

export function renderMarkdown(opts: RenderOptions): string {
    const lines: string[] = [];

    // Frontmatter
    const fmLines: string[] = [];
    fmLines.push(`title: ${opts.title}`);
    if (opts.type && opts.type !== "document") {
        fmLines.push(`type: ${opts.type}`);
    }

    // Exclude path-derived tags to avoid duplication on re-import
    const pathTags = opts.sourcePath ? new Set(tagsFromPath(opts.sourcePath)) : new Set<string>();
    const contentTags = opts.tags.filter(t => !pathTags.has(t));
    if (contentTags.length > 0) {
        fmLines.push("tags:");
        for (const tag of contentTags) {
            fmLines.push(`  - ${tag}`);
        }
    }

    if (opts.scope && Object.keys(opts.scope).length > 0) {
        fmLines.push("scope:");
        for (const [key, value] of Object.entries(opts.scope)) {
            fmLines.push(`  ${key}: ${value}`);
        }
    }

    lines.push("---");
    lines.push(...fmLines);
    lines.push("---");
    lines.push("");

    const prefix = "#".repeat(opts.splitLevel);

    for (const section of opts.sections) {
        const isIntro = section.title.endsWith(" — Introduction");

        if (!isIntro) {
            lines.push(`${prefix} ${section.title}`);
            lines.push("");
        }

        if (section.content) {
            lines.push(section.content);
            lines.push("");
        }

        if (section.rationale) {
            lines.push(`> **Rationale:** ${section.rationale}`);
            lines.push("");
        }
        if (section.alternatives && section.alternatives.length > 0) {
            lines.push("> **Alternatives:**");
            for (const alt of section.alternatives) {
                lines.push(`> - ${alt}`);
            }
            lines.push("");
        }
        if (section.consequences) {
            lines.push(`> **Consequences:** ${section.consequences}`);
            lines.push("");
        }
    }

    return lines.join("\n").trimEnd();
}
