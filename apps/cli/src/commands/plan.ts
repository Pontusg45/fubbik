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

interface PlanStep {
    id: string;
    description: string;
    status: string;
    order: number;
    note?: string | null;
}

interface Plan {
    id: string;
    title: string;
    description?: string | null;
    status: string;
    steps?: PlanStep[];
    totalSteps?: number;
    doneCount?: number;
    createdAt: string;
    updatedAt: string;
}

function progressBar(done: number, total: number): string {
    const pct = total === 0 ? 0 : (done / total) * 100;
    const filled = Math.round(pct / 5);
    return "█".repeat(filled) + "░".repeat(20 - filled) + ` ${Math.round(pct)}%`;
}

function stepIcon(status: string): string {
    switch (status) {
        case "done":
            return "✓";
        case "in_progress":
            return "→";
        case "blocked":
            return "✗";
        default:
            return "○";
    }
}

// ── Subcommands ─────────────────────────────────────────────────────

const createPlan = new Command("create")
    .description("Create a new plan")
    .requiredOption("-t, --title <title>", "plan title")
    .option("--description <desc>", "plan description")
    .option("--steps <steps...>", "initial steps")
    .action(async (opts: { title: string; description?: string; steps?: string[] }, cmd: Command) => {
        try {
            const body: Record<string, unknown> = { title: opts.title };
            if (opts.description) body.description = opts.description;
            if (opts.steps) {
                body.steps = opts.steps.map((desc, i) => ({ description: desc, order: i }));
            }

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
    .option("--status <status>", "filter by status")
    .action(async (opts: { status?: string }, cmd: Command) => {
        try {
            const params = new URLSearchParams();
            if (opts.status) params.set("status", opts.status);
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
                const total = plan.totalSteps ?? 0;
                const done = plan.doneCount ?? 0;
                const bar = progressBar(done, total);
                lines.push(
                    `  ${formatBold(plan.title)} ${formatDim(`(${plan.id})`)}`,
                    `    ${formatDim(plan.status)}  ${bar}  ${done}/${total} steps`,
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
    .description("Show plan details with steps")
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

            const steps = plan.steps ?? [];
            const done = steps.filter(s => s.status === "done").length;
            const bar = progressBar(done, steps.length);

            const lines = [
                formatBold(plan.title),
                formatDim(`ID: ${plan.id}  Status: ${plan.status}`),
                plan.description ? `\n${plan.description}` : "",
                "",
                `Progress: ${bar}  ${done}/${steps.length}`,
                ""
            ];

            if (steps.length > 0) {
                lines.push("Steps:");
                for (const step of steps.sort((a, b) => a.order - b.order)) {
                    const icon = stepIcon(step.status);
                    const note = step.note ? formatDim(` — ${step.note}`) : "";
                    lines.push(`  ${icon} ${step.description}${note} ${formatDim(`(${step.id})`)}`);
                }
            }

            output(cmd, plan, lines.join("\n"));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const stepDone = new Command("step-done")
    .description("Mark a step as done")
    .argument("<planId>", "plan ID")
    .argument("<stepId>", "step ID")
    .option("--note <note>", "completion note")
    .action(async (planId: string, stepId: string, opts: { note?: string }, cmd: Command) => {
        try {
            const body: Record<string, unknown> = { status: "done" };
            if (opts.note) body.note = opts.note;

            const res = await fetchApi(`/plans/${planId}/steps/${stepId}`, {
                method: "PATCH",
                body: JSON.stringify(body)
            });

            if (!res.ok) {
                outputError(`Failed to update step: ${res.status}`);
                process.exit(1);
            }

            const step = (await res.json()) as PlanStep;
            output(cmd, step, formatSuccess(`Step marked as done`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const addStep = new Command("add-step")
    .description("Add a step to a plan")
    .argument("<planId>", "plan ID")
    .argument("<description>", "step description")
    .action(async (planId: string, description: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/plans/${planId}/steps`, {
                method: "POST",
                body: JSON.stringify({ description })
            });

            if (!res.ok) {
                outputError(`Failed to add step: ${res.status}`);
                process.exit(1);
            }

            const step = (await res.json()) as PlanStep;
            outputQuiet(cmd, step.id);
            output(cmd, step, formatSuccess(`Added step "${step.description}" (${step.id})`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const activatePlan = new Command("activate")
    .description("Set plan status to active")
    .argument("<id>", "plan ID")
    .action(async (id: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/plans/${id}`, {
                method: "PATCH",
                body: JSON.stringify({ status: "active" })
            });

            if (!res.ok) {
                outputError(`Failed to activate plan: ${res.status}`);
                process.exit(1);
            }

            const plan = (await res.json()) as Plan;
            output(cmd, plan, formatSuccess(`Plan "${plan.title}" is now active`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const completePlan = new Command("complete")
    .description("Set plan status to completed")
    .argument("<id>", "plan ID")
    .action(async (id: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/plans/${id}`, {
                method: "PATCH",
                body: JSON.stringify({ status: "completed" })
            });

            if (!res.ok) {
                outputError(`Failed to complete plan: ${res.status}`);
                process.exit(1);
            }

            const plan = (await res.json()) as Plan;
            output(cmd, plan, formatSuccess(`Plan "${plan.title}" is now completed`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const exportPlan = new Command("export")
    .description("Export a plan as markdown")
    .argument("<id>", "plan ID")
    .action(async (id: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/plans/${id}`);
            if (!res.ok) {
                outputError(`Failed to get plan: ${res.status}`);
                process.exit(1);
            }

            const plan = (await res.json()) as Plan;
            const steps = plan.steps ?? [];

            // Generate superpowers plan markdown format
            const lines: string[] = [];
            lines.push(`# ${plan.title}`);
            lines.push("");
            lines.push("> **For agentic workers:** Use this plan to track implementation progress.");
            lines.push("");
            if (plan.description) {
                lines.push(`**Goal:** ${plan.description}`);
                lines.push("");
            }
            lines.push(`**Status:** ${plan.status}`);
            lines.push("");
            lines.push("---");
            lines.push("");

            // Group steps by task group (if bracketed) or flat
            let currentGroup = "";
            let taskNum = 0;

            for (const step of steps.sort((a, b) => a.order - b.order)) {
                const groupMatch = step.description.match(/^\[([^\]]+)\]\s*(.*)/);
                const group = groupMatch?.[1] ?? "";
                const desc = groupMatch?.[2] ?? step.description;

                if (group && group !== currentGroup) {
                    currentGroup = group;
                    taskNum++;
                    lines.push(`## Task ${taskNum}: ${group}`);
                    lines.push("");
                }

                const checkbox = step.status === "done" ? "[x]" : "[ ]";
                lines.push(`- ${checkbox} **${desc}**`);
                if (step.note) lines.push(`  ${step.note}`);
                lines.push("");
            }

            if (isJson(cmd)) {
                console.log(JSON.stringify({ markdown: lines.join("\n") }, null, 2));
            } else {
                console.log(lines.join("\n"));
            }
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
    .addCommand(stepDone)
    .addCommand(addStep)
    .addCommand(activatePlan)
    .addCommand(completePlan)
    .addCommand(exportPlan);
