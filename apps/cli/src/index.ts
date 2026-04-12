import { Command } from "commander";

// Group commands
import { chunkCommand } from "./commands/chunk";
import { codebaseCommand } from "./commands/codebase";
import { contextGroupCommand } from "./commands/context-group";
import { maintainCommand } from "./commands/maintain";
import { planCommand } from "./commands/plan";
import { reqCommand } from "./commands/req";
import { reviewCommand } from "./commands/review";
import { tagGroupCommand } from "./commands/tag-group";

// Remaining top-level commands
import { checkFilesCommand } from "./commands/check-files";
import { diffCommand } from "./commands/diff";
import { docsCommand } from "./commands/docs";
import { exportCommand } from "./commands/export";
import { exportSiteCommand } from "./commands/export-site";
import { gapsCommand } from "./commands/gaps";
import { generateCommand } from "./commands/generate";
import { hooksCommand } from "./commands/hooks";
import { importCommand } from "./commands/import";
import { initCommand } from "./commands/init";
import { kbDiffCommand } from "./commands/kb-diff";
import { mcpToolsCommand } from "./commands/mcp-tools";
import { openCommand } from "./commands/open";
import { promptCommand } from "./commands/prompt";
import { recapCommand } from "./commands/recap";
import { statsCommand } from "./commands/stats";
import { statusCommand } from "./commands/status";
import { suggestCommand } from "./commands/suggest";
import { syncCommand } from "./commands/sync";
import { syncClaudeMdCommand } from "./commands/sync-claude-md";
import { taskCommand } from "./commands/task";
import { watchCommand } from "./commands/watch";
import { whyCommand } from "./commands/why";

import { generateZshCompletions } from "./lib/completions";

const program = new Command();

program
    .name("fubbik")
    .description("A local-first knowledge framework for humans and machines")
    .version("0.0.1")
    .option("--json", "output as JSON (machine-readable)")
    .option("-q, --quiet", "minimal output (just IDs/values)");

// Groups
program.addCommand(chunkCommand);
program.addCommand(codebaseCommand);
program.addCommand(contextGroupCommand);
program.addCommand(maintainCommand);
program.addCommand(planCommand);
program.addCommand(reqCommand);
program.addCommand(reviewCommand);
program.addCommand(tagGroupCommand);

// Top-level
program.addCommand(checkFilesCommand);
program.addCommand(diffCommand);
program.addCommand(docsCommand);
program.addCommand(exportCommand);
program.addCommand(exportSiteCommand);
program.addCommand(gapsCommand);
program.addCommand(generateCommand);
program.addCommand(hooksCommand);
program.addCommand(importCommand);
program.addCommand(initCommand);
program.addCommand(kbDiffCommand);
program.addCommand(mcpToolsCommand);
program.addCommand(openCommand);
program.addCommand(promptCommand);
program.addCommand(recapCommand);
program.addCommand(statsCommand);
program.addCommand(statusCommand);
program.addCommand(suggestCommand);
program.addCommand(syncCommand);
program.addCommand(syncClaudeMdCommand);
program.addCommand(taskCommand);
program.addCommand(watchCommand);
program.addCommand(whyCommand);

program.command("completions")
    .description("Generate shell completions")
    .argument("<shell>", "shell type: zsh")
    .action((shell: string) => {
        if (shell === "zsh") {
            console.log(generateZshCompletions(program));
        } else {
            console.error(`Unsupported shell: ${shell}. Currently only zsh is supported.`);
        }
    });

program.parse();
