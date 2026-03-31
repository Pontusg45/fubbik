import { readdirSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { Command } from "commander";
import pc from "picocolors";
import { output, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";
import { resolveCodebaseId } from "../lib/detect-codebase";

function collectSourceFiles(dir: string, base: string): string[] {
    const files: string[] = [];
    const skipDirs = new Set(["node_modules", ".git", "dist", "build", ".next", ".turbo", ".claude", ".fubbik"]);
    const sourceExts = new Set([".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".rs", ".java", ".rb", ".vue", ".svelte"]);

    try {
        const entries = readdirSync(dir, { withFileTypes: true });
        for (const entry of entries) {
            if (entry.name.startsWith(".") && entry.isDirectory()) continue;
            if (skipDirs.has(entry.name)) continue;
            const fullPath = join(dir, entry.name);
            if (entry.isDirectory()) {
                files.push(...collectSourceFiles(fullPath, base));
            } else if (entry.isFile()) {
                const ext = entry.name.slice(entry.name.lastIndexOf("."));
                if (sourceExts.has(ext) && !entry.name.endsWith(".test.ts") && !entry.name.endsWith(".test.tsx") && !entry.name.endsWith(".spec.ts")) {
                    files.push(relative(base, fullPath));
                }
            }
        }
    } catch {
        // permission error, skip
    }
    return files;
}

export const gapsCommand = new Command("gaps")
    .description("Find files with no associated knowledge chunks")
    .argument("[directory]", "directory to scan", ".")
    .option("--codebase <name>", "codebase name")
    .option("--limit <n>", "max files to show", "30")
    .action(async (directory: string, opts: { codebase?: string; limit: string }, cmd: Command) => {
        const serverUrl = getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Run 'fubbik init' first.");
            process.exit(1);
        }

        const codebaseId = await resolveCodebaseId(serverUrl, { codebase: opts.codebase });
        const limit = parseInt(opts.limit, 10) || 30;

        // Collect all source files
        const absDir = resolve(directory);
        const allFiles = collectSourceFiles(absDir, absDir);

        if (allFiles.length === 0) {
            outputError("No source files found in " + directory);
            process.exit(1);
        }

        // Check each file against the context API to see if any chunks reference it
        const uncoveredFiles: string[] = [];
        const coveredFiles: string[] = [];

        for (const file of allFiles) {
            try {
                const params = new URLSearchParams({ path: file });
                if (codebaseId) params.set("codebaseId", codebaseId);
                const ctxRes = await fetch(`${serverUrl}/api/context/for-file?${params}`);
                if (ctxRes.ok) {
                    const ctx = (await ctxRes.json()) as any[];
                    if (ctx.length > 0) {
                        coveredFiles.push(file);
                    } else {
                        uncoveredFiles.push(file);
                    }
                } else {
                    uncoveredFiles.push(file);
                }
            } catch {
                uncoveredFiles.push(file);
            }
        }

        // Group uncovered by directory
        const byDir = new Map<string, string[]>();
        for (const f of uncoveredFiles) {
            const dir = f.includes("/") ? f.slice(0, f.lastIndexOf("/")) : ".";
            if (!byDir.has(dir)) byDir.set(dir, []);
            byDir.get(dir)!.push(f);
        }

        // Sort directories by number of uncovered files (most gaps first)
        const sortedDirs = [...byDir.entries()].sort((a, b) => b[1].length - a[1].length);

        const coverage = allFiles.length > 0 ? Math.round((coveredFiles.length / allFiles.length) * 100) : 0;

        const coverageColor = coverage >= 75 ? pc.green : coverage >= 50 ? pc.yellow : pc.red;

        const lines: string[] = [];
        lines.push(`Knowledge gaps in ${directory === "." ? "current directory" : directory}:`);
        lines.push(`  ${pc.bold(String(allFiles.length))} source files scanned`);
        lines.push(`  ${pc.bold(String(coveredFiles.length))} covered (${coverageColor(`${coverage}%`)})`);
        lines.push(`  ${pc.bold(String(uncoveredFiles.length))} with no knowledge`);
        lines.push("");

        if (sortedDirs.length > 0) {
            lines.push("Directories with most gaps:");
            let shown = 0;
            for (const [dir, files] of sortedDirs) {
                if (shown >= limit) break;
                lines.push(`  ${pc.bold(dir)}/ (${files.length} uncovered)`);
                for (const f of files.slice(0, 5)) {
                    lines.push(`    - ${pc.dim(f)}`);
                    shown++;
                    if (shown >= limit) break;
                }
                if (files.length > 5) {
                    lines.push(`    ... and ${files.length - 5} more`);
                }
            }
        }

        output(cmd, {
            directory,
            totalFiles: allFiles.length,
            coveredFiles: coveredFiles.length,
            uncoveredFiles: uncoveredFiles.length,
            coverage,
            gaps: sortedDirs.map(([dir, files]) => ({ directory: dir, count: files.length, files }))
        }, lines.join("\n"));
    });
