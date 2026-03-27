import { readFileSync } from "node:fs";

import { Command } from "commander";

import { resolveCodebaseId } from "../lib/detect-codebase";
import { output, outputError, outputQuiet } from "../lib/output";
import { getServerUrl } from "../lib/store";

interface ParsedRequirement {
    title: string;
    description: string;
    steps: Array<{ keyword: "given" | "when" | "then" | "and" | "but"; text: string }>;
}

function parseGherkin(content: string): ParsedRequirement[] {
    const lines = content.split("\n");
    const requirements: ParsedRequirement[] = [];
    let featureDesc = "";
    let current: ParsedRequirement | null = null;

    for (const raw of lines) {
        const line = raw.trim();
        if (line.startsWith("Feature:")) {
            featureDesc = line.replace("Feature:", "").trim();
        } else if (line.startsWith("Scenario:") || line.startsWith("Scenario Outline:")) {
            if (current) requirements.push(current);
            current = {
                title: line.replace(/Scenario(?: Outline)?:/, "").trim(),
                description: featureDesc,
                steps: [],
            };
        } else if (current) {
            const match = line.match(/^(Given|When|Then|And|But)\s+(.+)/i);
            if (match) {
                current.steps.push({
                    keyword: match[1]!.toLowerCase() as ParsedRequirement["steps"][number]["keyword"],
                    text: match[2]!,
                });
            }
        }
    }
    if (current) requirements.push(current);
    return requirements;
}

function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        console.error('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

export const importRequirementsCommand = new Command("import-requirements")
    .description("Import requirements from a Gherkin .feature file")
    .argument("<file>", "path to .feature file")
    .option("--codebase <name>", "scope to codebase")
    .option("--priority <p>", "default priority", "should")
    .action(async (file: string, opts: { codebase?: string; priority: string }, cmd: Command) => {
        const serverUrl = requireServer();
        const codebaseId = await resolveCodebaseId(serverUrl, { codebase: opts.codebase });

        let content: string;
        try {
            content = readFileSync(file, "utf-8");
        } catch {
            outputError(`Could not read file: ${file}`);
            return;
        }

        const requirements = parseGherkin(content);

        if (requirements.length === 0) {
            outputError("No scenarios found in the file.");
            return;
        }

        const created: Array<{ id: string; title: string }> = [];
        const failed: string[] = [];

        for (const req of requirements) {
            const body: Record<string, unknown> = {
                title: req.title,
                description: req.description,
                steps: req.steps,
                priority: opts.priority,
            };
            if (codebaseId) body.codebaseId = codebaseId;

            try {
                const res = await fetch(`${serverUrl}/api/requirements`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify(body),
                });

                if (!res.ok) {
                    failed.push(req.title);
                    continue;
                }

                const data = (await res.json()) as { id: string; title: string };
                created.push(data);
            } catch {
                failed.push(req.title);
            }
        }

        const ids = created.map(c => c.id);
        outputQuiet(cmd, ids.join("\n"));

        const lines: string[] = [];
        if (created.length > 0) {
            lines.push(`Imported ${created.length} requirement(s) from ${file}:\n`);
            for (const c of created) {
                lines.push(`  ${c.id}  ${c.title}`);
            }
        }
        if (failed.length > 0) {
            lines.push(`\nFailed to import ${failed.length} requirement(s):`);
            for (const title of failed) {
                lines.push(`  ${title}`);
            }
        }

        output(cmd, { created, failed, total: requirements.length }, lines.join("\n"));
    });
