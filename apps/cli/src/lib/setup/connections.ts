import type { DiscoveredChunk, DiscoveredConnection } from "./types";

/** Extract markdown link paths from content: [text](path) — non-http only */
function extractMarkdownLinks(content: string): string[] {
    const linkRe = /\[([^\]]*)\]\(([^)]+)\)/g;
    const paths: string[] = [];
    let m: RegExpExecArray | null;
    while ((m = linkRe.exec(content)) !== null) {
        const href = m[2]!;
        if (!href.startsWith("http://") && !href.startsWith("https://")) {
            // Normalize ./prefix
            paths.push(href.replace(/^\.\//, ""));
        }
    }
    return paths;
}

/** Extract bold-text keywords (**word**) from content */
function extractBoldKeywords(content: string): string[] {
    const boldRe = /\*\*([^*]+)\*\*/g;
    const keywords: string[] = [];
    let m: RegExpExecArray | null;
    while ((m = boldRe.exec(content)) !== null) {
        keywords.push(m[1]!.toLowerCase());
    }
    return keywords;
}

const GENERIC_TAGS = new Set(["documentation", "config", "docs", "reference", "convention"]);

/**
 * Infer connections between discovered chunks using 4 rules:
 * 1. Markdown links between tier-1 docs → "references"
 * 2. Tier-2 tech-stack chunks → monorepo structure chunk → "part_of"
 * 3. Routing chunks → database/auth chunks → "depends_on"
 * 4. Tier-2/3 keywords found in tier-1 content → "supports"
 */
export function inferConnections(chunks: DiscoveredChunk[]): DiscoveredConnection[] {
    const connections: DiscoveredConnection[] = [];
    const seen = new Set<string>();

    function add(sourceTitle: string, targetTitle: string, relation: string) {
        if (sourceTitle === targetTitle) return;
        const key = `${sourceTitle}→${targetTitle}→${relation}`;
        if (seen.has(key)) return;
        seen.add(key);
        connections.push({ sourceTitle, targetTitle, relation });
    }

    const tier1 = chunks.filter(c => c.tier === 1);
    const tier2 = chunks.filter(c => c.tier === 2);
    const tier3 = chunks.filter(c => c.tier === 3);

    // Build source → chunk map for markdown link resolution
    const bySource = new Map<string, DiscoveredChunk>();
    for (const chunk of chunks) {
        bySource.set(chunk.source, chunk);
    }

    // Rule 1: Markdown links between tier-1 docs
    for (const chunk of tier1) {
        const links = extractMarkdownLinks(chunk.content);
        for (const linkPath of links) {
            const target = bySource.get(linkPath);
            if (target && target.tier === 1) {
                add(chunk.title, target.title, "references");
            }
        }
    }

    // Rule 2: part_of — tier-2 tech-stack chunks → "Project Structure (Monorepo)"
    const monorepoStructure = chunks.find(
        c => c.tier === 2 && c.category === "structure" && c.title.includes("Monorepo"),
    );
    if (monorepoStructure) {
        for (const chunk of tier2) {
            if (chunk.category === "tech-stack") {
                add(chunk.title, monorepoStructure.title, "part_of");
            }
        }
    }

    // Rule 3: depends_on — routing depends_on database, routing depends_on auth
    const routingChunks = chunks.filter(c => c.tags.includes("routing"));
    const databaseChunks = chunks.filter(c => c.tags.includes("database"));
    const authChunks = chunks.filter(c => c.tags.includes("auth") || c.tags.includes("authentication"));

    for (const routing of routingChunks) {
        for (const db of databaseChunks) {
            add(routing.title, db.title, "depends_on");
        }
        for (const auth of authChunks) {
            add(routing.title, auth.title, "depends_on");
        }
    }

    // Rule 4: supports — keywords from tier-2/3 chunks found in tier-1 content
    const lowerTierChunks = [...tier2, ...tier3];
    for (const ltChunk of lowerTierChunks) {
        const boldKeywords = extractBoldKeywords(ltChunk.content);
        const nonGenericTags = ltChunk.tags.filter(t => !GENERIC_TAGS.has(t));
        const keywords = [...boldKeywords, ...nonGenericTags.map(t => t.toLowerCase())];

        if (keywords.length === 0) continue;

        for (const t1Chunk of tier1) {
            const lowerContent = t1Chunk.content.toLowerCase();
            const matched = keywords.some(kw => kw.length > 2 && lowerContent.includes(kw));
            if (matched) {
                add(ltChunk.title, t1Chunk.title, "supports");
            }
        }
    }

    return connections;
}
