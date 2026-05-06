import { extractFrontmatter, tagsFromPath } from "../chunks/parse-docs";

export interface MarkdownSection {
    title: string;
    content: string;
    order: number;
    rationale?: string;
    alternatives?: string[];
    consequences?: string;
}

export interface SplitResult {
    title: string;
    description?: string;
    tags: string[];
    sections: MarkdownSection[];
    splitLevel: number;
}

function detectSplitLevel(content: string): number {
    const match = content.match(/^#{2,6} /m);
    if (!match) return 2;
    return match[0].trimEnd().length;
}

interface DecisionContext {
    rationale?: string;
    alternatives?: string[];
    consequences?: string;
}

function extractDecisionContext(content: string): { cleanContent: string; context: DecisionContext } {
    const lines = content.split("\n");
    const context: DecisionContext = {};

    // Find where trailing blockquotes start (scan backwards from end)
    let trailingStart = lines.length;
    for (let i = lines.length - 1; i >= 0; i--) {
        const line = lines[i]!.trim();
        if (line === "" || line.startsWith(">")) {
            trailingStart = i;
        } else {
            break;
        }
    }

    const trailingBlock = lines.slice(trailingStart).join("\n");

    // Check if trailing block contains decision context markers
    const hasDecisionContext =
        trailingBlock.includes("> **Rationale:**") ||
        trailingBlock.includes("> **Alternatives:**") ||
        trailingBlock.includes("> **Consequences:**");

    if (!hasDecisionContext) {
        return { cleanContent: content, context };
    }

    // Parse decision context from trailing blockquotes
    const rationaleMatch = trailingBlock.match(/> \*\*Rationale:\*\*\s*(.*)/);
    if (rationaleMatch) context.rationale = rationaleMatch[1]!.trim();

    const altMatch = trailingBlock.match(/> \*\*Alternatives:\*\*\s*([\s\S]*?)(?=\n> \*\*(?:Rationale|Consequences):\*\*|\n[^>\n]|$)/);
    if (altMatch) {
        const altBlock = altMatch[0]!;
        const items = altBlock.match(/^>\s*-\s+(.+)$/gm);
        if (items) {
            context.alternatives = items.map(item => item.replace(/^>\s*-\s+/, "").trim());
        }
    }

    const consMatch = trailingBlock.match(/> \*\*Consequences:\*\*\s*(.*)/);
    if (consMatch) context.consequences = consMatch[1]!.trim();

    const cleanContent = lines.slice(0, trailingStart).join("\n").trimEnd();
    return { cleanContent, context };
}

export function splitMarkdown(raw: string, filePath: string, splitLevel?: 2 | 3 | 4 | "auto"): SplitResult {
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

    const fmTags = Array.isArray(frontmatter.tags) ? frontmatter.tags.map(String) : [];
    const pathTags = tagsFromPath(filePath);
    const tags = [...new Set([...fmTags, ...pathTags])];
    const description = (frontmatter.description as string) ?? undefined;

    const resolvedLevel: number =
        splitLevel === undefined || splitLevel === "auto"
            ? detectSplitLevel(content)
            : splitLevel;

    const prefix = "#".repeat(resolvedLevel);
    const headingRegex = new RegExp(`^${prefix} (.+)$`, "gm");
    const matches: { title: string; index: number }[] = [];
    let match: RegExpExecArray | null;
    while ((match = headingRegex.exec(content)) !== null) {
        matches.push({ title: match[1]!.trim(), index: match.index });
    }

    const sections: MarkdownSection[] = [];
    let orderCounter = 0;

    if (matches.length === 0) {
        const trimmed = content.trim();
        if (trimmed) {
            sections.push({ title: `${title} — Introduction`, content: trimmed, order: orderCounter });
        }
        return { title, description, tags, sections, splitLevel: resolvedLevel };
    }

    const preamble = content.slice(0, matches[0]!.index).trim();
    if (preamble && !/^#{2,6} /m.test(preamble)) {
        sections.push({ title: `${title} — Introduction`, content: preamble, order: orderCounter++ });
    }

    for (let i = 0; i < matches.length; i++) {
        const heading = matches[i]!;
        const nextIndex = i + 1 < matches.length ? matches[i + 1]!.index : content.length;
        const sectionContent = content
            .slice(heading.index + `${prefix} ${heading.title}`.length + 1, nextIndex)
            .trim();
        const { cleanContent, context } = extractDecisionContext(sectionContent);
        sections.push({
            title: heading.title,
            content: cleanContent,
            order: orderCounter++,
            ...context
        });
    }

    return { title, description, tags, sections, splitLevel: resolvedLevel };
}
