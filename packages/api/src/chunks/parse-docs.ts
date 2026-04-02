export interface ParsedDoc {
    title: string;
    content: string;
    type: string;
    tags: string[];
    scope?: Record<string, string>;
}

export function parseDocFile(path: string, raw: string): ParsedDoc {
    const { frontmatter, body } = extractFrontmatter(raw);

    // Title: frontmatter > first H1 > filename
    let title = frontmatter.title as string | undefined;
    let content = body;

    if (!title) {
        const headingMatch = content.match(/^#\s+(.+)$/m);
        if (headingMatch) {
            title = headingMatch[1]!.trim();
            // Remove the heading line from content
            content = content.replace(/^#\s+.+\n?/m, "").trim();
        }
    }

    if (!title) {
        // Derive from filename: "my-cool-notes.md" -> "my cool notes"
        const filename = path.split("/").pop() ?? path;
        title = filename.replace(/\.md$/i, "").replace(/[-_]/g, " ");
    }

    // Tags: frontmatter + folder path segments
    const fmTags = Array.isArray(frontmatter.tags) ? frontmatter.tags.map(String) : [];
    const folderTags = tagsFromPath(path);
    const tags = [...new Set([...fmTags, ...folderTags])];

    // Type: frontmatter or default "document"
    const type = (frontmatter.type as string) ?? "document";

    // Scope
    const scope =
        frontmatter.scope && typeof frontmatter.scope === "object" && !Array.isArray(frontmatter.scope)
            ? (frontmatter.scope as Record<string, string>)
            : undefined;

    return { title, content: content.trim(), type, tags, ...(scope ? { scope } : {}) };
}

function tagsFromPath(path: string): string[] {
    const parts = path.split("/");
    parts.pop(); // remove filename
    return parts.filter(Boolean);
}

export function extractFrontmatter(raw: string): { frontmatter: Record<string, unknown>; body: string } {
    const fmRegex = /^---\n([\s\S]*?)\n---\n?([\s\S]*)$/;
    const match = raw.match(fmRegex);
    if (!match) return { frontmatter: {}, body: raw };

    const yamlStr = match[1]!;
    const body = (match[2] ?? "").trim();

    // Simple YAML parser for the fields we care about
    const frontmatter: Record<string, unknown> = {};
    let currentKey: string | null = null;
    let currentArray: string[] | null = null;

    for (const line of yamlStr.split("\n")) {
        const arrayItemMatch = line.match(/^\s+-\s+(.+)$/);
        if (arrayItemMatch && currentKey) {
            if (!currentArray) currentArray = [];
            currentArray.push(arrayItemMatch[1]!.trim());
            continue;
        }

        // Flush previous array
        if (currentKey && currentArray) {
            frontmatter[currentKey] = currentArray;
            currentArray = null;
        }

        const kvMatch = line.match(/^(\w+):\s*(.*)$/);
        if (kvMatch) {
            currentKey = kvMatch[1]!;
            const value = kvMatch[2]!.trim();
            if (value) {
                frontmatter[currentKey] = value;
                currentKey = null; // not expecting array items
            }
            // If value is empty, might be followed by array items
        }

        // Nested object (scope)
        const nestedMatch = line.match(/^\s+(\w+):\s+(.+)$/);
        if (nestedMatch && currentKey && !arrayItemMatch) {
            if (typeof frontmatter[currentKey] !== "object" || Array.isArray(frontmatter[currentKey])) {
                frontmatter[currentKey] = {};
            }
            (frontmatter[currentKey] as Record<string, string>)[nestedMatch[1]!] = nestedMatch[2]!.trim();
        }
    }

    // Flush trailing array
    if (currentKey && currentArray) {
        frontmatter[currentKey] = currentArray;
    }

    return { frontmatter, body };
}
