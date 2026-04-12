import { execSync } from "node:child_process";

import { Command } from "commander";

import { fetchApi } from "../lib/api";
import { output, outputError, isJson } from "../lib/output";

export const contextForDiffCommand = new Command("for-diff")
    .description("Get context for files changed in git diff")
    .option("--staged", "diff staged changes only")
    .option("-t, --max-tokens <n>", "token budget", "8000")
    .option("-c, --codebase <id>", "codebase ID")
    .action(async (opts: { staged?: boolean; maxTokens: string; codebase?: string }, cmd: Command) => {
        try {
            const diffCmd = opts.staged ? "git diff --staged --name-only" : "git diff --name-only";
            const diffOutput = execSync(diffCmd, { encoding: "utf-8" }).trim();

            if (!diffOutput) {
                output(cmd, { files: [], chunks: [] }, "No changed files.");
                return;
            }

            const paths = diffOutput.split("\n").filter(Boolean);
            const params = new URLSearchParams({
                paths: paths.join(","),
                maxTokens: opts.maxTokens,
                format: isJson(cmd) ? "structured-json" : "structured-md",
            });
            if (opts.codebase) params.set("codebaseId", opts.codebase);

            const res = await fetchApi(`/context/for-files?${params}`);
            if (!res.ok) {
                outputError(`Failed: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const data = await res.json();
            const content = isJson(cmd)
                ? ""
                : `Context for ${paths.length} changed file(s):\n\n${(data as any).content ?? JSON.stringify(data, null, 2)}`;
            output(cmd, data, content);
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });
