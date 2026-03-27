import { Command } from "commander";

import { formatBold, formatDim, formatSuccess, formatType } from "../lib/colors";
import { isJson, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

async function fetchTaskApi(path: string, opts?: RequestInit) {
    const serverUrl = requireServer();
    const res = await fetch(`${serverUrl}/api${path}`, {
        ...opts,
        headers: { "Content-Type": "application/json", ...opts?.headers }
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    return res.json();
}

export const taskCommand = new Command("task")
    .description("Manage quick tasks for AI agents")

    .addCommand(
        new Command("add")
            .description("Add a task")
            .argument("<title>", "task title")
            .option("-d, --description <desc>", "description")
            .option("--codebase <name>", "codebase")
            .action(async (title, opts, cmd) => {
                try {
                    const task = (await fetchTaskApi("/tasks", {
                        method: "POST",
                        body: JSON.stringify({ title, description: opts.description })
                    })) as any;
                    if (isJson(cmd)) {
                        console.log(JSON.stringify(task, null, 2));
                        return;
                    }
                    console.error(formatSuccess(`Task created: ${formatBold(title)}`));
                    console.error(formatDim(`  ID: ${task.id}`));
                } catch (e: any) {
                    outputError(e.message);
                }
            })
    )

    .addCommand(
        new Command("list")
            .description("List open tasks")
            .action(async (_opts, cmd) => {
                try {
                    const tasks = await fetchTaskApi("/tasks");
                    if (isJson(cmd)) {
                        console.log(JSON.stringify(tasks, null, 2));
                        return;
                    }
                    if (!Array.isArray(tasks) || tasks.length === 0) {
                        console.error(formatDim("No open tasks"));
                        return;
                    }
                    for (const t of tasks) {
                        const step = t.steps?.[0];
                        const status = step?.status ?? t.status;
                        console.error(
                            `  ${formatType(status)} ${formatBold(t.title)} ${formatDim(`(${t.id.slice(0, 8)})`)}`
                        );
                    }
                } catch (e: any) {
                    outputError(e.message);
                }
            })
    )

    .addCommand(
        new Command("claim")
            .description("Claim a task (mark as in-progress)")
            .argument("<id>", "task ID")
            .action(async (id, _opts, cmd) => {
                try {
                    await fetchTaskApi(`/tasks/${id}/claim`, { method: "POST" });
                    if (!isJson(cmd)) console.error(formatSuccess("Task claimed"));
                } catch (e: any) {
                    outputError(e.message);
                }
            })
    )

    .addCommand(
        new Command("done")
            .description("Complete a task")
            .argument("<id>", "task ID")
            .option("-n, --note <note>", "completion note")
            .action(async (id, opts, cmd) => {
                try {
                    await fetchTaskApi(`/tasks/${id}/complete`, {
                        method: "POST",
                        body: JSON.stringify({ note: opts.note })
                    });
                    if (!isJson(cmd)) console.error(formatSuccess("Task completed"));
                } catch (e: any) {
                    outputError(e.message);
                }
            })
    );
