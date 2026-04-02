import { extractFrontmatter } from "../chunks/parse-docs";

export interface MarkdownSection {
    title: string;
    content: string;
    order: number;
}

export interface SplitResult {
    title: string;
    description?: string;
    tags: string[];
    sections: MarkdownSection[];
}

export function splitMarkdown(raw: string, filePath: string): SplitResult {
    const { frontmatter, body } = extractFrontmatter(raw);

    let title = frontmatter.title as string | undefined;
    let content = body;

    const h1Match = content.match(/^#\s+(.+)$/m);
    if (h1Match) {
        if (!title) title = h1Match[1]!.trim();
        content = content.replace(/^#\s+.+\n?/m, "").trim();
    }

    if (!title) {
        const filename = filePath.split("/").pop() ?? filePath;
        title = filename.replace(/\.md$/i, "").replace(/[-_]/g, " ");
    }

    const tags = Array.isArray(frontmatter.tags) ? frontmatter.tags.map(String) : [];
    const description = (frontmatter.description as string) ?? undefined;

    const h2Regex = /^## (.+)$/gm;
    const matches: { title: string; index: number }[] = [];
    let match: RegExpExecArray | null;
    while ((match = h2Regex.exec(content)) !== null) {
        matches.push({ title: match[1]!.trim(), index: match.index });
    }

    const sections: MarkdownSection[] = [];
    let orderCounter = 0;

    if (matches.length === 0) {
        const trimmed = content.trim();
        if (trimmed) {
            sections.push({ title: `${title} \u2014 Introduction`, content: trimmed, order: orderCounter });
        }
        return { title, description, tags, sections };
    }

    const preamble = content.slice(0, matches[0]!.index).trim();
    if (preamble) {
        sections.push({ title: `${title} \u2014 Introduction`, content: preamble, order: orderCounter++ });
    }

    for (let i = 0; i < matches.length; i++) {
        const heading = matches[i]!;
        const nextIndex = i + 1 < matches.length ? matches[i + 1]!.index : content.length;
        const sectionContent = content
            .slice(heading.index + `## ${heading.title}`.length + 1, nextIndex)
            .trim();
        sections.push({ title: heading.title, content: sectionContent, order: orderCounter++ });
    }

    return { title, description, tags, sections };
}
