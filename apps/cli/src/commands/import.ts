import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

import { Command } from "commander";

import { output, outputQuiet } from "../lib/output";
import { addChunk } from "../lib/store";

function parseFrontmatter(raw: string): { meta: Record<string, string>; content: string } {
    const match = raw.match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);
    if (!match) return { meta: {}, content: raw };

    const meta: Record<string, string> = {};
    const metaBlock = match[1] ?? "";
    for (const line of metaBlock.split("\n")) {
        const idx = line.indexOf(":");
        if (idx > 0) {
            meta[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
        }
    }
    // Strip leading "# Title" line from content
    let content = match[2] ?? "";
    const titleLine = content.match(/^# .+\n\n?/);
    if (titleLine) content = content.slice(titleLine[0].length);

    return { meta, content };
}

function parseTags(raw: string): string[] {
    const match = raw.match(/\[(.+)\]/);
    if (!match?.[1]) return [];
    return match[1].split(",").map(t => t.trim().replace(/^"|"$/g, ""));
}

export const importCommand = new Command("import")
    .description("Import chunks from a JSON file or markdown directory")
    .requiredOption("--file <path>", "path to JSON file or directory of .md files")
    .action((opts: { file: string }, cmd: Command) => {
        const stat = statSync(opts.file);
        const added: { id: string; title: string }[] = [];

        if (stat.isDirectory()) {
            const files = readdirSync(opts.file).filter(f => f.endsWith(".md"));
            for (const file of files) {
                const raw = readFileSync(join(opts.file, file), "utf-8");
                const { meta, content } = parseFrontmatter(raw);
                const title = meta.title?.replace(/^"|"$/g, "") ?? file.replace(".md", "");
                const chunk = addChunk({
                    title,
                    content: content.trim(),
                    type: meta.type ?? "note",
                    tags: meta.tags ? parseTags(meta.tags) : []
                });
                added.push({ id: chunk.id, title: chunk.title });
            }
        } else {
            const raw = readFileSync(opts.file, "utf-8");
            const chunks = JSON.parse(raw) as Array<{ title: string; content?: string; type?: string; tags?: string[] }>;
            for (const obj of chunks) {
                if (!obj.title) continue;
                const chunk = addChunk({
                    title: obj.title,
                    content: obj.content ?? "",
                    type: obj.type ?? "note",
                    tags: obj.tags ?? []
                });
                added.push({ id: chunk.id, title: chunk.title });
            }
        }

        outputQuiet(cmd, added.map(a => a.id).join("\n"));
        output(cmd, { added }, `✓ Imported ${added.length} chunk(s)`);
    });
