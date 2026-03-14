import { readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join, relative } from "node:path";

import { Command } from "commander";

import { output, outputQuiet } from "../lib/output";
import { addChunk } from "../lib/store";

function collectMarkdownFiles(dir: string, recursive: boolean): string[] {
    const files: string[] = [];
    const entries = readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
        const fullPath = join(dir, entry.name);
        if (entry.isDirectory() && recursive) {
            files.push(...collectMarkdownFiles(fullPath, true));
        } else if (entry.isFile() && entry.name.endsWith(".md")) {
            files.push(fullPath);
        }
    }
    return files;
}

function titleFromFilename(filename: string): string {
    return basename(filename, ".md").replace(/[-_]/g, " ");
}

function tagsFromPath(filePath: string, baseDir: string): string[] {
    const rel = relative(baseDir, filePath);
    const parts = rel.split("/");
    // Remove the filename, keep directory segments as tags
    parts.pop();
    return parts;
}

export const importDirCommand = new Command("import-dir")
    .description("Import all markdown files from a directory as chunks")
    .argument("<path>", "path to directory containing .md files")
    .option("--codebase <name>", "codebase name (unused, for documentation)")
    .option("--type <type>", "chunk type for all imported files", "document")
    .option("--no-recursive", "do not recurse into subdirectories")
    .action((dirPath: string, opts: { type: string; recursive: boolean }, cmd: Command) => {
        const stat = statSync(dirPath);
        if (!stat.isDirectory()) {
            console.error(`Error: ${dirPath} is not a directory`);
            process.exit(1);
        }

        const files = collectMarkdownFiles(dirPath, opts.recursive);
        const added: { id: string; title: string }[] = [];

        for (const filePath of files) {
            const content = readFileSync(filePath, "utf-8");
            const title = titleFromFilename(filePath);
            const tags = tagsFromPath(filePath, dirPath);

            const chunk = addChunk({
                title,
                content: content.trim(),
                type: opts.type,
                tags
            });
            added.push({ id: chunk.id, title: chunk.title });
        }

        outputQuiet(cmd, added.map(a => a.id).join("\n"));
        output(
            cmd,
            { added, fileCount: files.length },
            `Imported ${added.length} chunks from ${files.length} files`
        );
    });
