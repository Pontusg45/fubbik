import { Command } from "commander";

import { formatId, formatSuccess, formatTag, formatType } from "../lib/colors";
import { loadConfig } from "../lib/config";
import { output, outputError, outputQuiet } from "../lib/output";
import { openEditor, promptInput } from "../lib/prompt";
import { addChunk, getServerUrl } from "../lib/store";
import { getBuiltinTemplate, listBuiltinTemplateNames } from "../lib/templates";

export const addCommand = new Command("add")
    .description("Add a new chunk to the knowledge base")
    .option("-t, --title <title>", "chunk title")
    .option("-c, --content <content>", "chunk content", "")
    .option("--type <type>", "chunk type", "note")
    .option("--tags <tags>", "comma-separated tags", "")
    .option("--content-file <path>", "read content from file (use - for stdin)")
    .option("-i, --interactive", "interactive mode — prompt for each field")
    .option("--template <name>", "use a template (built-in or from server)")
    .option("--list-templates", "list available templates")
    .option("--global", "skip codebase scoping")
    .option("--codebase <name>", "scope to a specific codebase by name")
    .action(async (opts: {
        title?: string;
        content: string;
        type: string;
        tags: string;
        contentFile?: string;
        interactive?: boolean;
        template?: string;
        listTemplates?: boolean;
        global?: boolean;
        codebase?: string;
    }, cmd: Command) => {
        const config = loadConfig();

        // List templates and exit
        if (opts.listTemplates) {
            const builtinNames = listBuiltinTemplateNames();
            console.log("Built-in templates:");
            for (const name of builtinNames) console.log(`  ${name}`);
            try {
                const serverUrl = getServerUrl();
                if (serverUrl) {
                    const res = await fetch(`${serverUrl}/api/templates`);
                    if (res.ok) {
                        const templates = (await res.json()) as { name: string }[];
                        if (templates.length > 0) {
                            console.log("\nServer templates:");
                            for (const t of templates) console.log(`  ${t.name}`);
                        }
                    }
                }
            } catch {}
            return;
        }

        // Template support: check built-in first, then fall back to server
        let templateDefaults: { title?: string; content?: string; type?: string; tags?: string[] } = {};
        if (opts.template) {
            const builtin = getBuiltinTemplate(opts.template);
            if (builtin) {
                templateDefaults = {
                    type: builtin.type,
                    content: builtin.content,
                    tags: builtin.tags,
                };
            } else {
                // Fall back to server templates
                let serverUrl: string | undefined;
                try {
                    serverUrl = getServerUrl();
                } catch {}
                if (!serverUrl) {
                    const available = listBuiltinTemplateNames().join(", ");
                    outputError(`Template "${opts.template}" not found. Built-in templates: ${available}`);
                    return;
                }
                const res = await fetch(`${serverUrl}/api/templates`);
                if (!res.ok) {
                    outputError(`Failed to fetch templates: ${res.status}`);
                    return;
                }
                const templates = (await res.json()) as { name: string; type?: string; content?: string; tags?: string[] }[];
                const template = templates.find((t) => t.name.toLowerCase() === opts.template!.toLowerCase());
                if (!template) {
                    const available = [...listBuiltinTemplateNames(), ...templates.map(t => t.name)].join(", ");
                    outputError(`Template "${opts.template}" not found. Available: ${available}`);
                    return;
                }
                templateDefaults = { type: template.type, content: template.content, tags: template.tags };
            }
        }

        // Interactive mode
        if (opts.interactive) {
            const title = await promptInput("Title", opts.title || templateDefaults.title || "");
            if (!title) {
                outputError("Title is required.");
                return;
            }
            const type = await promptInput("Type", opts.type !== "note" ? opts.type : (templateDefaults.type || config.defaultType || "note"));
            const tagsInput = await promptInput("Tags (comma-separated)", opts.tags || (templateDefaults.tags || []).join(", "));
            const tags = tagsInput ? tagsInput.split(",").map(t => t.trim()).filter(Boolean) : [];

            const initialContent = templateDefaults.content || `# ${title}\n\n`;
            console.error("Opening editor for content...");
            const content = await openEditor(initialContent);

            const chunk = addChunk({ title, content, type, tags });
            outputQuiet(cmd, chunk.id);
            output(
                cmd,
                chunk,
                [
                    formatSuccess(`Created chunk ${formatId(chunk.id)}`),
                    `  Title: ${chunk.title}`,
                    `  Type: ${formatType(chunk.type)}`,
                    ...(tags.length > 0 ? [`  Tags: ${tags.map(formatTag).join(", ")}`] : [])
                ].join("\n")
            );
            return;
        }

        // Non-interactive: title is required
        if (!opts.title) {
            outputError("Title is required. Use -t <title> or -i for interactive mode.");
            return;
        }

        let content = opts.content || templateDefaults.content || "";
        if (opts.contentFile) {
            if (opts.contentFile === "-") {
                content = await Bun.stdin.text();
            } else {
                const { readFileSync } = await import("node:fs");
                content = readFileSync(opts.contentFile, "utf-8");
            }
        }

        const type = opts.type !== "note" ? opts.type : (templateDefaults.type || config.defaultType || opts.type);
        const explicitTags = opts.tags ? opts.tags.split(",").map(t => t.trim()) : [];
        const tags = explicitTags.length > 0 ? explicitTags : (templateDefaults.tags || []);

        const chunk = addChunk({ title: opts.title, content, type, tags });

        outputQuiet(cmd, chunk.id);
        output(
            cmd,
            chunk,
            [
                formatSuccess(`Created chunk ${formatId(chunk.id)}`),
                `  Title: ${chunk.title}`,
                `  Type: ${formatType(chunk.type)}`,
                ...(tags.length > 0 ? [`  Tags: ${tags.map(formatTag).join(", ")}`] : [])
            ].join("\n")
        );
    });
