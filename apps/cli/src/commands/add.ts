import { Command } from "commander";

import { formatId, formatSuccess, formatTag, formatType } from "../lib/colors";
import { output, outputError, outputQuiet } from "../lib/output";
import { openEditor, promptInput } from "../lib/prompt";
import { addChunk, getServerUrl } from "../lib/store";

export const addCommand = new Command("add")
    .description("Add a new chunk to the knowledge base")
    .option("-t, --title <title>", "chunk title")
    .option("-c, --content <content>", "chunk content", "")
    .option("--type <type>", "chunk type", "note")
    .option("--tags <tags>", "comma-separated tags", "")
    .option("--content-file <path>", "read content from file (use - for stdin)")
    .option("-i, --interactive", "interactive mode — prompt for each field")
    .option("--template <name>", "use a template (fetched from server)")
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
        global?: boolean;
        codebase?: string;
    }, cmd: Command) => {
        // Template support: fetch and merge template defaults
        let templateDefaults: { title?: string; content?: string; type?: string; tags?: string[] } = {};
        if (opts.template) {
            let serverUrl: string | undefined;
            try {
                serverUrl = getServerUrl();
            } catch {
                // store doesn't exist
            }
            if (!serverUrl) {
                outputError("Server URL required for templates. Run 'fubbik init'.");
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
                outputError(`Template "${opts.template}" not found. Available: ${templates.map(t => t.name).join(", ")}`);
                return;
            }
            templateDefaults = {
                type: template.type,
                content: template.content,
                tags: template.tags,
            };
        }

        // Interactive mode
        if (opts.interactive) {
            const title = await promptInput("Title", opts.title || templateDefaults.title || "");
            if (!title) {
                outputError("Title is required.");
                return;
            }
            const type = await promptInput("Type", opts.type !== "note" ? opts.type : (templateDefaults.type || "note"));
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

        const type = opts.type !== "note" ? opts.type : (templateDefaults.type || opts.type);
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
