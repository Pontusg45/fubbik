import { listChunksByTag } from "@fubbik/db/repository";
import { Effect } from "effect";

interface GenerateClaudeMdParams {
    userId: string;
    codebaseId?: string;
    tag?: string;
}

interface ChunkRow {
    id: string;
    title: string;
    content: string;
    type: string;
    rationale: string | null;
    summary: string | null;
}

const TYPE_SECTIONS: Record<string, string> = {
    note: "Conventions",
    document: "Architecture",
    reference: "References"
};

function sectionLabel(type: string): string {
    return TYPE_SECTIONS[type] ?? "Other";
}

function formatChunkEntry(c: ChunkRow): string {
    const parts = [`### ${c.title}`];
    if (c.content) parts.push(c.content);
    if (c.rationale) parts.push(`**Rationale:** ${c.rationale}`);
    return parts.join("\n\n");
}

export function generateClaudeMd(params: GenerateClaudeMdParams) {
    const tagName = params.tag ?? "claude-context";

    return listChunksByTag({
        userId: params.userId,
        tagName,
        codebaseId: params.codebaseId
    }).pipe(
        Effect.map(chunks => {
            if (chunks.length === 0) {
                return { content: `# Project Context\n\nNo chunks found with tag "${tagName}".`, chunks: 0 };
            }

            // Group by section
            const sections = new Map<string, ChunkRow[]>();
            for (const c of chunks) {
                const label = sectionLabel(c.type);
                const group = sections.get(label) ?? [];
                group.push(c);
                sections.set(label, group);
            }

            // Build markdown
            const parts: string[] = ["# Project Context\n"];
            const sectionOrder = ["Conventions", "Architecture", "References", "Other"];
            for (const sectionName of sectionOrder) {
                const group = sections.get(sectionName);
                if (!group || group.length === 0) continue;
                parts.push(`## ${sectionName}\n`);
                for (const c of group) {
                    parts.push(formatChunkEntry(c));
                }
            }

            return { content: parts.join("\n\n"), chunks: chunks.length };
        })
    );
}
