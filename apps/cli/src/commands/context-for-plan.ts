import { Command } from "commander";

import { fetchApi } from "../lib/api";
import { output, outputError, isJson } from "../lib/output";

export const contextForPlanCommand = new Command("for-plan")
    .description("Get context scoped to a plan (analyze chunks, requirements, tasks)")
    .argument("<planId>", "plan ID")
    .option("-t, --max-tokens <n>", "token budget", "8000")
    .option("-c, --codebase <id>", "codebase ID")
    .action(async (planId: string, opts: { maxTokens: string; codebase?: string }, cmd: Command) => {
        try {
            const params = new URLSearchParams({
                planId,
                maxTokens: opts.maxTokens,
                format: isJson(cmd) ? "structured-json" : "structured-md",
            });
            if (opts.codebase) params.set("codebaseId", opts.codebase);

            const res = await fetchApi(`/context/for-plan?${params}`);
            if (!res.ok) {
                outputError(`Failed: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const data = await res.json();
            const content = isJson(cmd) ? "" : ((data as any).content ?? JSON.stringify(data, null, 2));
            output(cmd, data, content);
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });
