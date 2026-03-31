import { Command } from "commander";
import pc from "picocolors";
import { output, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";
import { resolveCodebaseId } from "../lib/detect-codebase";

export const whyCommand = new Command("why")
    .description("Show reasoning and decisions behind a file")
    .argument("<path>", "file path to explain")
    .option("--codebase <name>", "codebase name")
    .action(async (filePath: string, opts: { codebase?: string }, cmd: Command) => {
        const serverUrl = getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Run 'fubbik init' first.");
            process.exit(1);
        }

        const codebaseId = await resolveCodebaseId(serverUrl, { codebase: opts.codebase });

        // Fetch context chunks for this file
        const params = new URLSearchParams({ path: filePath });
        if (codebaseId) params.set("codebaseId", codebaseId);

        const res = await fetch(`${serverUrl}/api/context/for-file?${params}`);
        if (!res.ok) {
            outputError(`Failed to fetch context: ${res.status}`);
            process.exit(1);
        }

        const allChunks = (await res.json()) as any[];

        // Filter to chunks that have reasoning content:
        // - chunks with rationale, alternatives, or consequences
        // - chunks of type "convention" (often contain decisions)
        // - chunks with tags like "architecture", "decision", "convention"
        const decisionTags = new Set(["architecture", "decision", "convention", "rationale", "why"]);

        const reasoningChunks = allChunks.filter((c: any) => {
            if (c.rationale || c.alternatives?.length || c.consequences) return true;
            if (c.type === "convention") return true;
            const tags = (c.tags ?? []).map((t: any) => typeof t === "string" ? t : t.name);
            if (tags.some((t: string) => decisionTags.has(t.toLowerCase()))) return true;
            return false;
        });

        // Also include chunks that reference this file explicitly (they chose to link to it)
        const contextChunks = allChunks.filter((c: any) => !reasoningChunks.includes(c));

        const lines: string[] = [];
        lines.push(`Why: ${pc.bold(filePath)}`);
        lines.push("");

        if (reasoningChunks.length === 0 && contextChunks.length === 0) {
            lines.push("No knowledge found for this file.");
            lines.push("Run 'fubbik suggest " + filePath + "' to see what chunks could be created.");
            output(cmd, { path: filePath, reasoning: [], context: [] }, lines.join("\n"));
            return;
        }

        if (reasoningChunks.length > 0) {
            lines.push(pc.bold(`Decisions & Conventions (${reasoningChunks.length}):`));
            lines.push("");
            for (const c of reasoningChunks) {
                lines.push(`  [${pc.cyan(c.type)}] ${c.title}`);
                if (c.matchReason) lines.push(`    Match: ${pc.dim(c.matchReason)}`);
                if (c.rationale) {
                    const rationale = c.rationale.length > 120 ? c.rationale.slice(0, 120) + "..." : c.rationale;
                    lines.push(`    Rationale: ${pc.dim(rationale)}`);
                }
                if (c.alternatives?.length) {
                    lines.push(`    Alternatives considered: ${c.alternatives.join(", ")}`);
                }
                if (c.consequences) {
                    const cons = c.consequences.length > 120 ? c.consequences.slice(0, 120) + "..." : c.consequences;
                    lines.push(`    Consequences: ${cons}`);
                }
                lines.push("");
            }
        }

        if (contextChunks.length > 0) {
            lines.push(pc.bold(`Related context (${contextChunks.length}):`));
            for (const c of contextChunks) {
                lines.push(`  [${pc.cyan(c.type)}] ${c.title} (${pc.dim(c.matchReason ?? "related")})`);
            }
        }

        output(cmd, { path: filePath, reasoning: reasoningChunks, context: contextChunks }, lines.join("\n"));
    });
