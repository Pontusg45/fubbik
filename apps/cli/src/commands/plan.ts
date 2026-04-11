import { Command } from "commander";

import { formatBold, formatDim, formatSuccess } from "../lib/colors";
import { isJson, output, outputError, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

// ── Helpers ─────────────────────────────────────────────────────────

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

async function fetchApi(path: string, opts?: RequestInit): Promise<Response> {
    const serverUrl = requireServer();
    return fetch(`${serverUrl}/api${path}`, {
        ...opts,
        headers: {
            "Content-Type": "application/json",
            ...opts?.headers
        }
    });
}

interface PlanTask {
    id: string;
    title: string;
    status: string;
    order: number;
    description?: string | null;
}

interface Plan {
    id: string;
    title: string;
    description?: string | null;
    status: string;
    tasks?: PlanTask[];
    totalTasks?: number;
    doneCount?: number;
    createdAt: string;
    updatedAt: string;
}

function progressBar(done: number, total: number): string {
    const pct = total === 0 ? 0 : (done / total) * 100;
    const filled = Math.round(pct / 5);
    return "█".repeat(filled) + "░".repeat(20 - filled) + ` ${Math.round(pct)}%`;
}

function taskIcon(status: string): string {
    switch (status) {
        case "done":
            return "✓";
        case "in_progress":
            return "→";
        case "blocked":
            return "✗";
        case "skipped":
            return "-";
        default:
            return "○";
    }
}

// ── Subcommands ─────────────────────────────────────────────────────

const createPlan = new Command("create")
    .description("Create a new plan")
    .argument("<title>", "plan title")
    .option("-d, --description <desc>", "plan description")
    .option("-c, --codebase <codebaseId>", "codebase ID")
    .action(async (title: string, opts: { description?: string; codebase?: string }, cmd: Command) => {
        try {
            const body: Record<string, unknown> = { title };
            if (opts.description) body.description = opts.description;
            if (opts.codebase) body.codebaseId = opts.codebase;

            const res = await fetchApi("/plans", {
                method: "POST",
                body: JSON.stringify(body)
            });

            if (!res.ok) {
                outputError(`Failed to create plan: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const plan = (await res.json()) as Plan;
            outputQuiet(cmd, plan.id);
            output(cmd, plan, formatSuccess(`Created plan "${plan.title}" (${plan.id})`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const listPlans = new Command("list")
    .description("List plans with progress")
    .option("-s, --status <status>", "filter by status (draft, analyzing, ready, in_progress, completed, archived)")
    .option("-c, --codebase <codebaseId>", "filter by codebase ID")
    .option("--archived", "include archived plans")
    .action(async (opts: { status?: string; codebase?: string; archived?: boolean }, cmd: Command) => {
        try {
            const params = new URLSearchParams();
            if (opts.status) params.set("status", opts.status);
            if (opts.codebase) params.set("codebaseId", opts.codebase);
            if (opts.archived) params.set("includeArchived", "true");
            const qs = params.toString();

            const res = await fetchApi(`/plans${qs ? `?${qs}` : ""}`);
            if (!res.ok) {
                outputError(`Failed to list plans: ${res.status}`);
                process.exit(1);
            }

            const plans = (await res.json()) as Plan[];

            if (isJson(cmd)) {
                console.log(JSON.stringify(plans, null, 2));
                return;
            }

            outputQuiet(cmd, plans.map(p => p.id).join("\n"));

            if (plans.length === 0) {
                output(cmd, plans, "No plans found.");
                return;
            }

            const lines: string[] = [];
            for (const plan of plans) {
                const total = plan.totalTasks ?? 0;
                const done = plan.doneCount ?? 0;
                const bar = progressBar(done, total);
                lines.push(
                    `  ${formatBold(plan.title)} ${formatDim(`(${plan.id})`)}`,
                    `    ${formatDim(plan.status)}  ${bar}  ${done}/${total} tasks`,
                    ""
                );
            }
            output(cmd, plans, lines.join("\n"));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const showPlan = new Command("show")
    .description("Show plan details with tasks")
    .argument("<id>", "plan ID")
    .action(async (id: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/plans/${id}`);
            if (!res.ok) {
                outputError(`Failed to get plan: ${res.status}`);
                process.exit(1);
            }

            const plan = (await res.json()) as Plan;

            if (isJson(cmd)) {
                console.log(JSON.stringify(plan, null, 2));
                return;
            }

            const tasks = plan.tasks ?? [];
            const done = tasks.filter(t => t.status === "done").length;
            const bar = progressBar(done, tasks.length);

            const lines = [
                formatBold(plan.title),
                formatDim(`ID: ${plan.id}  Status: ${plan.status}`),
                plan.description ? `\n${plan.description}` : "",
                "",
                `Progress: ${bar}  ${done}/${tasks.length}`,
                ""
            ];

            if (tasks.length > 0) {
                lines.push("Tasks:");
                for (const task of tasks.sort((a, b) => a.order - b.order)) {
                    const icon = taskIcon(task.status);
                    lines.push(`  ${icon} ${task.title} ${formatDim(`(${task.id})`)}`);
                }
            }

            output(cmd, plan, lines.join("\n"));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const setStatus = new Command("status")
    .description("Update plan status (draft, analyzing, ready, in_progress, completed, archived)")
    .argument("<planId>", "plan ID")
    .argument("<status>", "new status")
    .action(async (planId: string, status: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/plans/${planId}`, {
                method: "PATCH",
                body: JSON.stringify({ status })
            });

            if (!res.ok) {
                outputError(`Failed to update plan status: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const plan = (await res.json()) as Plan;
            output(cmd, plan, formatSuccess(`Plan "${plan.title}" status set to ${plan.status}`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const addTask = new Command("add-task")
    .description("Add a task to a plan")
    .argument("<planId>", "plan ID")
    .argument("<title>", "task title")
    .option("-d, --description <description>", "task description")
    .action(async (planId: string, title: string, opts: { description?: string }, cmd: Command) => {
        try {
            const body: Record<string, unknown> = { title };
            if (opts.description) body.description = opts.description;

            const res = await fetchApi(`/plans/${planId}/tasks`, {
                method: "POST",
                body: JSON.stringify(body)
            });

            if (!res.ok) {
                outputError(`Failed to add task: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const task = (await res.json()) as PlanTask;
            outputQuiet(cmd, task.id);
            output(cmd, task, formatSuccess(`Added task "${task.title}" (${task.id})`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const taskDone = new Command("task-done")
    .description("Mark a task as done")
    .argument("<planId>", "plan ID")
    .argument("<taskId>", "task ID")
    .action(async (planId: string, taskId: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/plans/${planId}/tasks/${taskId}`, {
                method: "PATCH",
                body: JSON.stringify({ status: "done" })
            });

            if (!res.ok) {
                outputError(`Failed to update task: ${res.status}`);
                process.exit(1);
            }

            const task = (await res.json()) as PlanTask;
            output(cmd, task, formatSuccess(`Task marked as done`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const linkRequirement = new Command("link-requirement")
    .description("Link a requirement to a plan")
    .argument("<planId>", "plan ID")
    .argument("<requirementId>", "requirement ID")
    .action(async (planId: string, requirementId: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/plans/${planId}/requirements`, {
                method: "POST",
                body: JSON.stringify({ requirementId })
            });

            if (!res.ok) {
                outputError(`Failed to link requirement: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            output(cmd, { planId, requirementId }, formatSuccess("Requirement linked."));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

// ── Export ───────────────────────────────────────────────────────────

export const planCommand = new Command("plan")
    .description("Manage plans")
    .addCommand(createPlan)
    .addCommand(listPlans)
    .addCommand(showPlan)
    .addCommand(setStatus)
    .addCommand(addTask)
    .addCommand(taskDone)
    .addCommand(linkRequirement);
