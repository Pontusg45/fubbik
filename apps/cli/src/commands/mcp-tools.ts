import { Command } from "commander";
import pc from "picocolors";

interface ToolInfo {
    name: string;
    description: string;
}

interface ToolGroup {
    label: string;
    tools: ToolInfo[];
}

const TOOL_GROUPS: ToolGroup[] = [
    {
        label: "Core",
        tools: [
            { name: "search_chunks", description: "Search chunks by query, codebase, or tags" },
            { name: "get_chunk", description: "Get full chunk details by ID" },
            { name: "create_chunk", description: "Create a new knowledge chunk" },
            { name: "update_chunk", description: "Update an existing knowledge chunk" },
            { name: "get_conventions", description: "Get convention/rationale chunks for a codebase" },
            { name: "get_requirements", description: "Get requirements for a codebase" },
            { name: "search_vocabulary", description: "Search vocabulary entries for a codebase" },
        ],
    },
    {
        label: "Plans",
        tools: [
            { name: "list_plan_templates", description: "List available plan templates" },
            { name: "create_plan", description: "Create plan with optional template" },
            { name: "list_plans", description: "List plans with optional status filter" },
            { name: "get_plan", description: "Get plan details with steps and progress" },
            { name: "update_plan_step", description: "Update a step's status or add a note" },
            { name: "add_plan_step", description: "Add a new step to a plan" },
            { name: "complete_plan", description: "Mark a plan as completed" },
            { name: "import_plan_markdown", description: "Import a plan from markdown content" },
            { name: "create_plan_from_requirements", description: "Generate plan from selected requirements" },
        ],
    },
    {
        label: "Sessions",
        tools: [
            { name: "begin_implementation", description: "Start implementation session with context bundle" },
            { name: "record_chunk_reference", description: "Record a chunk referenced during implementation" },
            { name: "record_assumption", description: "Record an assumption due to missing knowledge" },
            { name: "record_requirement_addressed", description: "Record that a requirement was implemented" },
            { name: "complete_implementation", description: "Complete session and generate review brief" },
            { name: "resolve_assumption_as_chunk", description: "Resolve assumption by creating a chunk from it" },
            { name: "mark_plan_step", description: "Mark a plan step as done" },
            { name: "get_plan_progress", description: "Get progress of plan linked to a session" },
        ],
    },
    {
        label: "Requirements",
        tools: [
            { name: "list_requirements", description: "List requirements with status/priority filters" },
            { name: "create_requirement", description: "Create requirement with BDD steps" },
            { name: "update_requirement_status", description: "Update requirement status" },
        ],
    },
    {
        label: "Suggestions",
        tools: [
            { name: "suggest_requirements", description: "Get context to suggest new requirements" },
            { name: "create_requirements_batch", description: "Batch create requirements with use cases" },
        ],
    },
    {
        label: "Context",
        tools: [
            { name: "sync_claude_md", description: "Generate CLAUDE.md from tagged chunks" },
        ],
    },
    {
        label: "Tasks",
        tools: [
            { name: "add_task", description: "Add a quick task for tracking" },
            { name: "list_tasks", description: "List open tasks" },
            { name: "complete_task", description: "Complete a task" },
        ],
    },
];

export const mcpToolsCommand = new Command("mcp-tools")
    .description("List available MCP server tools")
    .option("--json", "Output as JSON")
    .action((opts: { json?: boolean }) => {
        if (opts.json) {
            const allTools = TOOL_GROUPS.flatMap(g =>
                g.tools.map(t => ({ group: g.label, ...t }))
            );
            console.log(JSON.stringify(allTools, null, 2));
            return;
        }

        const lines: string[] = [];
        lines.push(pc.bold("MCP Server Tools:"));
        lines.push("");

        for (const group of TOOL_GROUPS) {
            lines.push(`${pc.bold(group.label)}:`);
            const maxName = Math.max(...group.tools.map(t => t.name.length));
            for (const tool of group.tools) {
                lines.push(`  ${pc.cyan(tool.name.padEnd(maxName + 2))}${tool.description}`);
            }
            lines.push("");
        }

        const total = TOOL_GROUPS.reduce((sum, g) => sum + g.tools.length, 0);
        lines.push(pc.dim(`${total} tools across ${TOOL_GROUPS.length} groups`));

        console.log(lines.join("\n"));
    });
