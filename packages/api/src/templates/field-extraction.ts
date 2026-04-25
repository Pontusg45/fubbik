import { matchHeading } from "./match-engine";
import type { ParsedHeading, FieldMapping, ExtractedFields } from "./types";

export interface ExtractionResult {
    extracted: ExtractedFields;
    remainingContent: string;
}

interface Section {
    heading: string | null;
    level: number | null;
    content: string;
}

// --- Public API ---

/**
 * Scan a markdown string for all headings (levels 1–6).
 */
export function parseHeadings(markdown: string): ParsedHeading[] {
    const headings: ParsedHeading[] = [];
    const lines = markdown.split("\n");
    for (const line of lines) {
        const m = /^(#{1,6})\s+(.+)$/.exec(line);
        if (m) {
            headings.push({ level: m[1]!.length, text: m[2]!.trim() });
        }
    }
    return headings;
}

/**
 * Extract structured fields from a markdown document using the provided
 * field mappings.  Returns extracted fields plus the remaining markdown
 * (everything that was NOT pulled into a specific field).
 */
export function extractFields(
    markdown: string,
    mappings: FieldMapping[],
): ExtractionResult {
    if (mappings.length === 0) {
        return { extracted: {}, remainingContent: markdown };
    }

    const sections = splitIntoSections(markdown);
    const extracted: ExtractedFields = {};
    // Track which section indices have been consumed (removed from body).
    const consumed = new Set<number>();

    for (const mapping of mappings) {
        // Find first section whose heading matches any of the mapping's patterns.
        const idx = sections.findIndex((sec) => {
            if (sec.heading === null) return false;
            const ph: ParsedHeading = { text: sec.heading, level: sec.level ?? 2 };
            return mapping.headings.some((pattern) =>
                matchHeading(pattern, ph, mapping.match),
            );
        });

        if (idx === -1) continue;

        const sec = sections[idx]!;
        const content = sec.content.trim();

        if (mapping.target === "content") {
            // "content" target — stays in the body, don't consume.
            continue;
        }

        // Mark as consumed so it won't appear in remainingContent.
        consumed.add(idx);

        switch (mapping.target) {
            case "rationale":
            case "consequences":
            case "summary":
                (extracted as Record<string, unknown>)[mapping.target] = content;
                break;
            case "alternatives":
                extracted.alternatives = splitBullets(content);
                break;
            case "scope":
                extracted.scope = parseKeyValueLines(content);
                break;
        }
    }

    // Rebuild remaining content from non-consumed sections, preserving order.
    const remaining = sections
        .filter((_, i) => !consumed.has(i))
        .map((sec) => {
            if (sec.heading === null) return sec.content;
            const hashes = "#".repeat(sec.level ?? 2);
            return `${hashes} ${sec.heading}\n${sec.content}`;
        })
        .join("")
        .trimEnd();

    return { extracted, remainingContent: remaining || markdown };
}

// --- Internal helpers ---

/**
 * Split markdown into sections at H2+ heading boundaries.
 * The first "section" (before any H2+) captures leading content
 * (title + preamble).
 */
function splitIntoSections(markdown: string): Section[] {
    const lines = markdown.split("\n");
    const sections: Section[] = [];
    let currentHeading: string | null = null;
    let currentLevel: number | null = null;
    let buffer: string[] = [];

    for (const line of lines) {
        const m = /^(#{2,6})\s+(.+)$/.exec(line);
        if (m) {
            // Save whatever we've accumulated so far.
            sections.push({
                heading: currentHeading,
                level: currentLevel,
                content: buffer.join("\n"),
            });
            currentHeading = m[2]!.trim();
            currentLevel = m[1]!.length;
            buffer = [];
        } else {
            buffer.push(line);
        }
    }

    // Push the last section.
    sections.push({
        heading: currentHeading,
        level: currentLevel,
        content: buffer.join("\n"),
    });

    return sections;
}

/**
 * Split content by `- ` bullet lines.  Returns all non-empty items.
 * Falls back to wrapping the whole content as a single-item array.
 */
function splitBullets(content: string): string[] {
    const bullets = content
        .split("\n")
        .filter((l) => l.trimStart().startsWith("- "))
        .map((l) => l.replace(/^\s*-\s+/, "").trim())
        .filter(Boolean);

    if (bullets.length > 0) return bullets;

    // No bullet lines — return the content itself as a single item (if non-empty).
    return content.trim() ? [content.trim()] : [];
}

/**
 * Parse `key: value` lines into a Record.
 */
function parseKeyValueLines(content: string): Record<string, string> {
    const result: Record<string, string> = {};
    for (const line of content.split("\n")) {
        const colonIdx = line.indexOf(":");
        if (colonIdx === -1) continue;
        const key = line.slice(0, colonIdx).trim();
        const value = line.slice(colonIdx + 1).trim();
        if (key) result[key] = value;
    }
    return result;
}
