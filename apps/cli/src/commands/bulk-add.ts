import { readFileSync } from "node:fs";
import { Command } from "commander";

import { addChunk } from "../lib/store";
import { output, outputQuiet } from "../lib/output";

export const bulkAddCommand = new Command("bulk-add")
    .description("Import chunks from a JSONL file (one JSON object per line)")
    .requiredOption("--file <path>", "path to JSONL file (use - for stdin)")
    .action(async (opts: { file: string }, cmd: Command) => {
        let raw: string;
        if (opts.file === "-") {
            raw = await Bun.stdin.text();
        } else {
            raw = readFileSync(opts.file, "utf-8");
        }

        const lines = raw.split("\n").filter(l => l.trim());
        const added: { id: string; title: string }[] = [];
        const errors: { line: number; error: string }[] = [];

        for (let i = 0; i < lines.length; i++) {
            try {
                const obj = JSON.parse(lines[i]!) as { title?: string; content?: string; type?: string; tags?: string[] };
                if (!obj.title) {
                    errors.push({ line: i + 1, error: "missing title" });
                    continue;
                }
                const chunk = addChunk({
                    title: obj.title,
                    content: obj.content ?? "",
                    type: obj.type ?? "note",
                    tags: obj.tags ?? []
                });
                added.push({ id: chunk.id, title: chunk.title });
            } catch (e) {
                errors.push({ line: i + 1, error: (e as Error).message });
            }
        }

        outputQuiet(cmd, added.map(a => a.id).join("\n"));
        output(cmd, { added, errors }, [
            `✓ Added ${added.length} chunk(s)`,
            ...(errors.length > 0 ? [`✗ ${errors.length} error(s):`] : []),
            ...errors.map(e => `  Line ${e.line}: ${e.error}`)
        ].join("\n"));

        if (errors.length > 0) process.exit(1);
    });
