import type { ScoredChunk } from "./utils";

export interface ChunkWithMetadata extends ScoredChunk {
    healthScore: number;
    isStale: boolean;
    hasPendingProposal: boolean;
}

export interface ContextSection {
    title: string;
    chunks: ChunkWithMetadata[];
}

export interface StructuredContext {
    sections: ContextSection[];
    totalChunks: number;
}

// Map chunk type (+ optional convention tag) to section title
function sectionTitle(chunk: ChunkWithMetadata): string {
    if (chunk.type === "note" && chunk.tags.includes("convention")) return "Conventions";
    switch (chunk.type) {
        case "note":
            return "Notes";
        case "document":
            return "Architecture";
        case "reference":
            return "API Reference";
        case "schema":
            return "Schemas";
        case "checklist":
            return "Checklists";
        default:
            return chunk.type.charAt(0).toUpperCase() + chunk.type.slice(1);
    }
}

export function formatStructured(chunks: ChunkWithMetadata[]): StructuredContext {
    const sectionMap = new Map<string, ChunkWithMetadata[]>();

    for (const chunk of chunks) {
        const title = sectionTitle(chunk);
        const existing = sectionMap.get(title) ?? [];
        existing.push(chunk);
        sectionMap.set(title, existing);
    }

    const sections: ContextSection[] = [];
    for (const [title, sectionChunks] of sectionMap.entries()) {
        sections.push({ title, chunks: sectionChunks });
    }

    return { sections, totalChunks: chunks.length };
}

// Governing behavior rule linked to a file via the behavior matrix.
export interface GoverningBehavior {
    ruleId: string;
    ruleTitle: string;
    description: string | null;
    rationale: string | null;
    counterexample: string | null;
    matrixId: string;
    matrixName: string;
    layer: string;
    dimensionName: string;
    kind: string;
    ref: string;
}

/**
 * Render the "Behaviors governing this file" section. Returns an empty string
 * when there are no behaviors so callers can append unconditionally.
 */
export function formatBehaviorsMarkdown(behaviors: GoverningBehavior[]): string {
    if (behaviors.length === 0) return "";

    const lines: string[] = ["## Behaviors governing this file", ""];

    // De-duplicate by rule (a rule can match a path via multiple code links).
    const seen = new Set<string>();
    for (const b of behaviors) {
        if (seen.has(b.ruleId)) continue;
        seen.add(b.ruleId);

        const desc = b.description ? ` — ${b.description}` : "";
        lines.push(`- [${b.layer}/${b.matrixName}] ${b.ruleTitle}${desc}`);
        if (b.counterexample) {
            lines.push(`  ↳ counterexample: ${b.counterexample}`);
        }
    }

    return lines.join("\n").trimEnd();
}

export function formatStructuredMarkdown(ctx: StructuredContext): string {
    const lines: string[] = ["# Project Context", ""];

    for (const section of ctx.sections) {
        lines.push(`## ${section.title}`, "");
        for (const chunk of section.chunks) {
            const flags: string[] = [];
            if (chunk.isStale) flags.push("⚠ STALE");
            if (chunk.hasPendingProposal) flags.push("⚠ PENDING PROPOSAL");
            const flagStr = flags.length > 0 ? ` ${flags.join(" ")}` : "";
            lines.push(`### ${chunk.title} [health: ${chunk.healthScore}]${flagStr}`);
            if (chunk.content) {
                lines.push("", chunk.content);
            }
            if (chunk.rationale) {
                lines.push("", `**Rationale:** ${chunk.rationale}`);
            }
            lines.push("");
        }
    }

    return lines.join("\n").trimEnd();
}
