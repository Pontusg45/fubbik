import { Command } from "commander";
import pc from "picocolors";
import { output, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

function requireServer() {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        outputError("No server URL configured. Run 'fubbik init' first.");
        process.exit(1);
    }
    return serverUrl;
}

/** Convert relative date string to number of days for the API */
function parseToDays(since: string): number {
    const match = since.match(/^(\d+)([dhwm])$/);
    if (!match) return 7; // default 7 days
    const [, num, unit] = match;
    const multiplier = { d: 1, h: 1 / 24, w: 7, m: 30 }[unit!] ?? 1;
    return Math.ceil(Number(num) * multiplier);
}

export const recapCommand = new Command("recap")
    .description("Summarize knowledge base changes since a date")
    .option("--since <date>", "date or relative (e.g., 7d, 2w, 1m)", "7d")
    .option("--codebase <name>", "filter by codebase")
    .action(async (opts: { since: string; codebase?: string }, cmd: Command) => {
        const serverUrl = requireServer();
        const days = parseToDays(opts.since);

        // Fetch chunks updated since the date (API expects days as number)
        const params = new URLSearchParams({ after: String(days), sort: "updated", limit: "200" });
        if (opts.codebase) {
            // resolve codebase name to id
            const cbRes = await fetch(`${serverUrl}/api/codebases`);
            const codebases = (await cbRes.json()) as { id: string; name: string }[];
            const match = codebases.find(c => c.name === opts.codebase);
            if (match) params.set("codebaseId", match.id);
        }

        const res = await fetch(`${serverUrl}/api/chunks?${params}`);
        if (!res.ok) {
            outputError(`Failed to fetch chunks: ${res.status}`);
            process.exit(1);
        }

        const data = (await res.json()) as { chunks: any[]; total: number };
        const chunks = data.chunks ?? data;

        // Separate new vs updated (heuristic: created == updated means new)
        const newChunks = chunks.filter((c: any) => c.createdAt === c.updatedAt);
        const updatedChunks = chunks.filter((c: any) => c.createdAt !== c.updatedAt);

        // Group by type
        const byType = new Map<string, any[]>();
        for (const c of chunks) {
            const type = c.type ?? "note";
            if (!byType.has(type)) byType.set(type, []);
            byType.get(type)!.push(c);
        }

        // Build human-readable summary
        const lines: string[] = [];
        lines.push(`Knowledge base recap (last ${days} day${days !== 1 ? "s" : ""}):`);
        const since = new Date(Date.now() - days * 86400000).toISOString();
        lines.push(`  ${newChunks.length} new chunk${newChunks.length !== 1 ? "s" : ""}, ${updatedChunks.length} updated`);
        lines.push("");

        if (newChunks.length > 0) {
            lines.push(pc.bold("New:"));
            for (const c of newChunks.slice(0, 20)) {
                const tags = c.tags?.map((t: any) => typeof t === "string" ? t : t.name).join(", ") ?? "";
                lines.push(`  + [${pc.cyan(c.type)}] ${c.title}${tags ? ` (${pc.yellow(tags)})` : ""}`);
            }
            if (newChunks.length > 20) lines.push(`  ... and ${newChunks.length - 20} more`);
            lines.push("");
        }

        if (updatedChunks.length > 0) {
            lines.push(pc.bold("Updated:"));
            for (const c of updatedChunks.slice(0, 10)) {
                lines.push(`  ~ [${pc.cyan(c.type)}] ${c.title}`);
            }
            if (updatedChunks.length > 10) lines.push(`  ... and ${updatedChunks.length - 10} more`);
            lines.push("");
        }

        if (byType.size > 0) {
            lines.push(pc.bold("By type:"));
            for (const [type, items] of byType) {
                lines.push(`  ${type}: ${pc.bold(String(items.length))}`);
            }
        }

        output(cmd, { since, newChunks: newChunks.length, updatedChunks: updatedChunks.length, byType: Object.fromEntries(byType.entries()), chunks }, lines.join("\n"));
    });
