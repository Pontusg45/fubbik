import { Command } from "commander";

import { fetchApi } from "../lib/api";
import { output, outputError } from "../lib/output";

const createCmd = new Command("create")
    .description("Create a frozen context snapshot for AI agent use")
    .option("--plan <id>", "plan ID to snapshot context for")
    .option("--task <id>", "task ID within the plan")
    .option("--about <concept>", "concept or topic to snapshot context about")
    .option("--files <csv>", "comma-separated file paths to snapshot context for")
    .option("--max-tokens <n>", "token budget", "8000")
    .option("--codebase <id>", "codebase ID to scope context")
    .action(async (opts: { plan?: string; task?: string; about?: string; files?: string; maxTokens: string; codebase?: string }, cmd: Command) => {
        try {
            const body: Record<string, unknown> = {
                maxTokens: Number(opts.maxTokens),
            };
            if (opts.plan) body.planId = opts.plan;
            if (opts.task) body.taskId = opts.task;
            if (opts.about) body.concept = opts.about;
            if (opts.files) body.filePaths = opts.files.split(",").map(f => f.trim()).filter(Boolean);
            if (opts.codebase) body.codebaseId = opts.codebase;

            const res = await fetchApi("/context/snapshot", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(body),
            });
            if (!res.ok) {
                outputError(`Failed: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const data = (await res.json()) as { snapshotId: string; tokenCount: number; chunkCount: number; createdAt: string };
            output(cmd, data, `Snapshot created: ${data.snapshotId}\nChunks: ${data.chunkCount}  Tokens: ${data.tokenCount}\nCreated: ${data.createdAt}`);
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const getCmd = new Command("get")
    .description("Retrieve a frozen context snapshot by ID")
    .argument("<snapshotId>", "snapshot ID")
    .action(async (snapshotId: string, _opts: unknown, cmd: Command) => {
        try {
            const res = await fetchApi(`/context/snapshot/${snapshotId}`);
            if (!res.ok) {
                outputError(`Failed: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const data = (await res.json()) as {
                id: string;
                query: unknown;
                chunks: Array<{ title: string; content: string; type: string; rationale?: string | null }>;
                tokenCount: number;
                createdAt: string;
            };

            const lines: string[] = [
                `# Context Snapshot: ${data.id}`,
                `Created: ${data.createdAt}  Tokens: ${data.tokenCount}  Chunks: ${data.chunks.length}`,
                "",
            ];
            for (const chunk of data.chunks) {
                lines.push(`## ${chunk.title} [${chunk.type}]`);
                if (chunk.content) lines.push("", chunk.content);
                if (chunk.rationale) lines.push("", `Rationale: ${chunk.rationale}`);
                lines.push("");
            }

            output(cmd, data, lines.join("\n").trimEnd());
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const listCmd = new Command("list")
    .description("List your context snapshots")
    .action(async (_opts: unknown, cmd: Command) => {
        try {
            const res = await fetchApi("/context/snapshots");
            if (!res.ok) {
                outputError(`Failed: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const data = (await res.json()) as Array<{ id: string; tokenCount: number; createdAt: string; query: unknown }>;

            if (data.length === 0) {
                output(cmd, data, "No snapshots found.");
                return;
            }

            const lines = data.map(s => `${s.id}  tokens:${s.tokenCount}  ${new Date(s.createdAt).toLocaleString()}`);
            output(cmd, data, lines.join("\n"));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const deleteCmd = new Command("delete")
    .description("Delete a context snapshot")
    .argument("<snapshotId>", "snapshot ID")
    .action(async (snapshotId: string) => {
        try {
            const res = await fetchApi(`/context/snapshot/${snapshotId}`, { method: "DELETE" });
            if (!res.ok) {
                outputError(`Failed: ${res.status} ${await res.text()}`);
                process.exit(1);
            }
            console.log(`Deleted snapshot ${snapshotId}`);
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

export const contextSnapshotCommand = new Command("snapshot")
    .description("Manage frozen context snapshots for AI agents")
    .addCommand(createCmd)
    .addCommand(getCmd)
    .addCommand(listCmd)
    .addCommand(deleteCmd);
