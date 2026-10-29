import { Command } from "commander";

import { resolveSpaceId } from "../lib/detect-space";
import { output, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

function parseStep(raw: string): { keyword: string; text: string } {
    const colonIdx = raw.indexOf(":");
    if (colonIdx === -1) {
        console.error(`Invalid step format: "${raw}". Expected "keyword: text".`);
        process.exit(1);
    }
    return {
        keyword: raw.slice(0, colonIdx).trim().toLowerCase(),
        text: raw.slice(colonIdx + 1).trim()
    };
}

const listRequirements = new Command("list")
    .description("List requirements")
    .option("--status <status>", "filter by status (passing, failing, untested)")
    .option("-s, --space <name>", "scope to space")
    .option("--codebase <name>", "alias for --space (deprecated)")
    .option("--priority <priority>", "filter by priority")
    .action(async (opts: { status?: string; space?: string; codebase?: string; priority?: string }, cmd: Command) => {
        const serverUrl = requireServer();
        const spaceId = await resolveSpaceId(serverUrl, { space: opts.space ?? opts.codebase });

        const params = new URLSearchParams();
        if (opts.status) params.set("status", opts.status);
        if (opts.priority) params.set("priority", opts.priority);
        if (spaceId) params.set("spaceId", spaceId);

        const res = await fetch(`${serverUrl}/api/requirements?${params}`);
        if (!res.ok) {
            console.error(`Failed to list requirements: ${res.status}`);
            process.exit(1);
        }

        const data = (await res.json()) as {
            id: string;
            title: string;
            status?: string;
            priority?: string;
        }[];
        outputQuiet(cmd, data.map(r => r.id).join("\n"));

        if (data.length === 0) {
            output(cmd, data, "No requirements found.");
        } else {
            const lines = [`${data.length} requirement(s):\n`];
            for (const req of data) {
                const status = req.status ? ` [${req.status}]` : "";
                const priority = req.priority ? ` (${req.priority})` : "";
                lines.push(`  ${req.id}  ${req.title}${status}${priority}`);
            }
            output(cmd, data, lines.join("\n"));
        }
    });

const addRequirement = new Command("add")
    .description("Add a new requirement")
    .argument("<title>", "requirement title")
    .option("--step <step...>", 'step in "keyword: text" format (e.g. "given: a user is logged in")')
    .option("-s, --space <name>", "scope to space")
    .option("--codebase <name>", "alias for --space (deprecated)")
    .action(async (title: string, opts: { step?: string[]; space?: string; codebase?: string }, cmd: Command) => {
        const serverUrl = requireServer();
        const spaceId = await resolveSpaceId(serverUrl, { space: opts.space ?? opts.codebase });

        const steps = (opts.step ?? []).map(parseStep);

        const body: Record<string, unknown> = { title, steps };
        if (spaceId) body.spaceId = spaceId;

        const res = await fetch(`${serverUrl}/api/requirements`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body)
        });

        if (!res.ok) {
            const text = await res.text();
            console.error(`Failed to create requirement: ${res.status} ${text}`);
            process.exit(1);
        }

        const data = (await res.json()) as { id: string; title: string };
        outputQuiet(cmd, data.id);
        output(cmd, data, `Created requirement "${data.title}" (${data.id})`);
    });

const statusRequirement = new Command("status")
    .description("Update requirement status")
    .argument("<id>", "requirement ID")
    .argument("<status>", "new status (passing, failing, untested)")
    .action(async (id: string, status: string, _opts: Record<string, unknown>, cmd: Command) => {
        const serverUrl = requireServer();

        const validStatuses = ["passing", "failing", "untested"];
        if (!validStatuses.includes(status)) {
            console.error(`Invalid status "${status}". Must be one of: ${validStatuses.join(", ")}`);
            process.exit(1);
        }

        const res = await fetch(`${serverUrl}/api/requirements/${id}/status`, {
            method: "PATCH",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ status })
        });

        if (!res.ok) {
            const text = await res.text();
            console.error(`Failed to update status: ${res.status} ${text}`);
            process.exit(1);
        }

        const data = (await res.json()) as { id: string; status: string };
        outputQuiet(cmd, data.id);
        output(cmd, data, `Requirement ${data.id} status set to "${data.status}"`);
    });

const exportRequirements = new Command("export")
    .description("Export requirements in a given format")
    .option("--format <format>", "export format (gherkin, vitest, markdown)", "gherkin")
    .option("-s, --space <name>", "scope to space")
    .option("--codebase <name>", "alias for --space (deprecated)")
    .action(async (opts: { format: string; space?: string; codebase?: string }, cmd: Command) => {
        const serverUrl = requireServer();
        const spaceId = await resolveSpaceId(serverUrl, { space: opts.space ?? opts.codebase });

        const validFormats = ["gherkin", "vitest", "markdown"];
        if (!validFormats.includes(opts.format)) {
            console.error(`Invalid format "${opts.format}". Must be one of: ${validFormats.join(", ")}`);
            process.exit(1);
        }

        const params = new URLSearchParams();
        params.set("format", opts.format);
        if (spaceId) params.set("spaceId", spaceId);

        const res = await fetch(`${serverUrl}/api/requirements/export?${params}`);
        if (!res.ok) {
            const text = await res.text();
            console.error(`Failed to export requirements: ${res.status} ${text}`);
            process.exit(1);
        }

        const text = await res.text();
        output(cmd, text, text);
    });

const verifyRequirements = new Command("verify")
    .description("Verify requirements and check for cross-ref warnings")
    .option("-s, --space <name>", "scope to space")
    .option("--codebase <name>", "alias for --space (deprecated)")
    .action(async (opts: { space?: string; codebase?: string }, cmd: Command) => {
        const serverUrl = requireServer();
        const spaceId = await resolveSpaceId(serverUrl, { space: opts.space ?? opts.codebase });

        const params = new URLSearchParams();
        if (spaceId) params.set("spaceId", spaceId);

        const res = await fetch(`${serverUrl}/api/requirements?${params}`);
        if (!res.ok) {
            console.error(`Failed to fetch requirements: ${res.status}`);
            process.exit(1);
        }

        const data = (await res.json()) as {
            id: string;
            title: string;
            status?: string;
            warnings?: string[];
        }[];

        const issues: string[] = [];
        for (const req of data) {
            if (req.warnings && req.warnings.length > 0) {
                for (const warning of req.warnings) {
                    issues.push(`  ${req.id}  ${req.title}: ${warning}`);
                }
            }
            if (req.status === "failing") {
                issues.push(`  ${req.id}  ${req.title}: status is failing`);
            }
            if (req.status === "untested") {
                issues.push(`  ${req.id}  ${req.title}: status is untested`);
            }
        }

        const result = { total: data.length, issues: issues.length, details: issues };
        outputQuiet(cmd, issues.length.toString());

        if (issues.length === 0) {
            output(cmd, result, `All ${data.length} requirement(s) verified. No issues found.`);
        } else {
            const lines = [`${issues.length} issue(s) found across ${data.length} requirement(s):\n`];
            lines.push(...issues);
            output(cmd, result, lines.join("\n"));
        }
    });

export const requirementsCommand = new Command("requirements")
    .description("Manage requirements")
    .addCommand(listRequirements)
    .addCommand(addRequirement)
    .addCommand(statusRequirement)
    .addCommand(exportRequirements)
    .addCommand(verifyRequirements);
