import { Command } from "commander";

import { formatBold, formatDim, formatSuccess } from "../lib/colors";
import { fetchApi } from "../lib/api";
import { isJson, output, outputError, outputQuiet } from "../lib/output";

interface Matrix {
    id: string;
    name: string;
    layer: string;
    description: string | null;
}

interface ViewResponse {
    matrix: Matrix;
    dimensions: Array<{ id: string; name: string; order: number }>;
    rules: Array<{ id: string; title: string; category: string | null; order: number }>;
    cells: Record<string, { id: string; status: string; requirementCount: number } | null>;
    summary: { specified: number; unspecified: number; violated: number; total: number };
}

// ── Subcommands ─────────────────────────────────────────────────────

const listMatrices = new Command("list")
    .description("List matrices")
    .option("--layer <layer>", "Filter by layer (invariant|contract)")
    .action(async (opts: { layer?: string }, cmd: Command) => {
        try {
            const params = new URLSearchParams();
            if (opts.layer) params.set("layer", opts.layer);
            const qs = params.toString();

            const res = await fetchApi(`/matrices${qs ? `?${qs}` : ""}`);
            if (!res.ok) {
                outputError(`Failed to list matrices: ${res.status}`);
                process.exit(1);
            }

            const matrices = (await res.json()) as Matrix[];

            if (isJson(cmd)) {
                console.log(JSON.stringify(matrices, null, 2));
                return;
            }

            outputQuiet(cmd, matrices.map(m => m.id).join("\n"));

            if (matrices.length === 0) {
                output(cmd, matrices, "No matrices found.");
                return;
            }

            const lines: string[] = [];
            for (const m of matrices) {
                lines.push(`  ${formatBold(m.name)} [${m.layer}] ${formatDim(`(${m.id})`)}`);
                if (m.description) lines.push(`    ${m.description}`);
            }
            output(cmd, matrices, lines.join("\n"));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const createMatrix = new Command("create")
    .description("Create a new matrix")
    .argument("<name>", "matrix name")
    .requiredOption("--layer <layer>", "Layer: invariant or contract")
    .option("--description <desc>", "Description")
    .option("--codebase <id>", "Codebase ID")
    .action(async (name: string, opts: { layer: string; description?: string; codebase?: string }, cmd: Command) => {
        try {
            const body: Record<string, unknown> = { name, layer: opts.layer };
            if (opts.description) body.description = opts.description;
            if (opts.codebase) body.codebaseId = opts.codebase;

            const res = await fetchApi("/matrices", {
                method: "POST",
                body: JSON.stringify(body)
            });

            if (!res.ok) {
                outputError(`Failed to create matrix: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const matrix = (await res.json()) as Matrix;
            outputQuiet(cmd, matrix.id);
            output(cmd, matrix, formatSuccess(`Created matrix "${matrix.name}" (${matrix.id})`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const showMatrix = new Command("show")
    .description("Show matrix as ASCII grid")
    .argument("<id>", "matrix ID")
    .action(async (id: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/matrices/${id}/view`);
            if (!res.ok) {
                outputError(`Failed to get matrix: ${res.status}`);
                process.exit(1);
            }

            const view = (await res.json()) as ViewResponse;

            if (isJson(cmd)) {
                console.log(JSON.stringify(view, null, 2));
                return;
            }

            const { matrix, dimensions, rules, cells, summary } = view;

            const lines: string[] = [
                "",
                `${formatBold(matrix.name)} [${matrix.layer}]`,
                `Coverage: ${summary.specified} specified, ${summary.unspecified} unspecified, ${summary.violated} violated / ${summary.total} total`,
                ""
            ];

            if (dimensions.length === 0 || rules.length === 0) {
                lines.push("Matrix is empty. Add dimensions and rules first.");
                output(cmd, view, lines.join("\n"));
                return;
            }

            const maxRuleLen = Math.max(...rules.map(r => r.title.length), 10);
            const colWidth = Math.max(...dimensions.map(d => d.name.length), 5);

            const header = "".padEnd(maxRuleLen + 2) + dimensions.map(d => d.name.padStart(colWidth)).join(" ");
            lines.push(header);
            lines.push("-".repeat(header.length));

            for (const rule of rules) {
                const row = rule.title.padEnd(maxRuleLen + 2) + dimensions.map(dim => {
                    const key = `${rule.id}:${dim.id}`;
                    const cell = cells[key];
                    if (!cell) return ".".padStart(colWidth);
                    const symbol = cell.status === "specified" ? "✓" : cell.status === "violated" ? "✗" : "?";
                    return symbol.padStart(colWidth);
                }).join(" ");
                lines.push(row);
            }

            output(cmd, view, lines.join("\n"));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const addDimension = new Command("add-dimension")
    .description("Add a dimension (column)")
    .argument("<matrixId>", "matrix ID")
    .argument("<name>", "dimension name")
    .action(async (matrixId: string, name: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/matrices/${matrixId}/dimensions`, {
                method: "POST",
                body: JSON.stringify({ name })
            });

            if (!res.ok) {
                outputError(`Failed to add dimension: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const dim = (await res.json()) as { id: string; name: string };
            outputQuiet(cmd, dim.id);
            output(cmd, dim, formatSuccess(`Added dimension "${name}"`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const addRule = new Command("add-rule")
    .description("Add a rule (row)")
    .argument("<matrixId>", "matrix ID")
    .argument("<title>", "rule title")
    .option("--category <category>", "Category for grouping")
    .action(async (matrixId: string, title: string, opts: { category?: string }, cmd: Command) => {
        try {
            const body: Record<string, unknown> = { title };
            if (opts.category) body.category = opts.category;

            const res = await fetchApi(`/matrices/${matrixId}/rules`, {
                method: "POST",
                body: JSON.stringify(body)
            });

            if (!res.ok) {
                outputError(`Failed to add rule: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const rule = (await res.json()) as { id: string; title: string };
            outputQuiet(cmd, rule.id);
            output(cmd, rule, formatSuccess(`Added rule "${title}"`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const cellToggle = new Command("cell")
    .description("Toggle a cell (mark as relevant or remove)")
    .argument("<matrixId>", "matrix ID")
    .argument("<ruleId>", "rule ID")
    .argument("<dimensionId>", "dimension ID")
    .action(async (matrixId: string, ruleId: string, dimensionId: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/matrices/${matrixId}/cells`, {
                method: "PUT",
                body: JSON.stringify({ ruleId, dimensionId })
            });

            if (!res.ok) {
                outputError(`Failed to toggle cell: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const result = (await res.json()) as { action: string };
            output(cmd, result, formatSuccess(`Cell ${result.action}`));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const showGaps = new Command("gaps")
    .description("List unspecified and violated cells")
    .argument("<id>", "matrix ID")
    .action(async (id: string, _opts: Record<string, unknown>, cmd: Command) => {
        try {
            const res = await fetchApi(`/matrices/${id}/view`);
            if (!res.ok) {
                outputError(`Failed to get matrix: ${res.status}`);
                process.exit(1);
            }

            const view = (await res.json()) as ViewResponse;

            if (isJson(cmd)) {
                const gaps = Object.entries(view.cells)
                    .filter(([, cell]) => cell && (cell.status === "unspecified" || cell.status === "violated"))
                    .map(([key, cell]) => ({ key, ...cell }));
                console.log(JSON.stringify(gaps, null, 2));
                return;
            }

            const { dimensions, rules, cells } = view;
            const dimMap = new Map(dimensions.map(d => [d.id, d.name]));
            const ruleMap = new Map(rules.map(r => [r.id, r.title]));

            const lines: string[] = [];
            let count = 0;
            for (const [key, cell] of Object.entries(cells)) {
                if (cell && (cell.status === "unspecified" || cell.status === "violated")) {
                    const parts = key.split(":");
                    const ruleId = parts[0] ?? "";
                    const dimId = parts[1] ?? "";
                    const status = cell.status === "violated" ? "VIOLATED" : "GAP";
                    lines.push(`[${status}] "${ruleMap.get(ruleId)}" × "${dimMap.get(dimId)}"`)
;
                    count++;
                }
            }

            if (count === 0) {
                output(cmd, { count: 0 }, formatSuccess("No gaps or violations found."));
            } else {
                lines.push("", `${count} issue(s) found.`);
                output(cmd, { count }, lines.join("\n"));
            }
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

const linkRequirement = new Command("link")
    .description("Link a requirement to a cell")
    .argument("<cellId>", "cell ID")
    .argument("<requirementId>", "requirement ID")
    .requiredOption("--matrix <matrixId>", "Matrix ID")
    .action(async (cellId: string, requirementId: string, opts: { matrix: string }, cmd: Command) => {
        try {
            const res = await fetchApi(`/matrices/${opts.matrix}/cells/${cellId}/requirements`, {
                method: "POST",
                body: JSON.stringify({ requirementId })
            });

            if (!res.ok) {
                outputError(`Failed to link requirement: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            output(cmd, { cellId, requirementId }, formatSuccess("Requirement linked to cell"));
        } catch (err) {
            outputError(String(err));
            process.exit(1);
        }
    });

// ── Export ───────────────────────────────────────────────────────────

export const matrixCommand = new Command("matrix")
    .description("Manage behavioral specification matrices")
    .addCommand(listMatrices)
    .addCommand(createMatrix)
    .addCommand(showMatrix)
    .addCommand(addDimension)
    .addCommand(addRule)
    .addCommand(cellToggle)
    .addCommand(showGaps)
    .addCommand(linkRequirement);
