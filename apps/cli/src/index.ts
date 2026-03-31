import { Command } from "commander";

import { addCommand } from "./commands/add";
import { bulkAddCommand } from "./commands/bulk-add";
import { catCommand } from "./commands/cat";
import { checkFilesCommand } from "./commands/check-files";
import { codebaseCommand } from "./commands/codebase";
import { contextCommand } from "./commands/context";
import { contextDirCommand } from "./commands/context-dir";
import { contextForCommand } from "./commands/context-for";
import { diffCommand } from "./commands/diff";
import { doctorCommand } from "./commands/doctor";
import { enrichCommand } from "./commands/enrich";
import { exportCommand } from "./commands/export";
import { exportSiteCommand } from "./commands/export-site";
import { gapsCommand } from "./commands/gaps";
import { generateCommand } from "./commands/generate";
import { requirementsCommand } from "./commands/requirements";
import { getCommand } from "./commands/get";
import { importRequirementsCommand } from "./commands/import-requirements";
import { kbDiffCommand } from "./commands/kb-diff";
import { watchCommand } from "./commands/watch";
import { healthCommand } from "./commands/health";
import { hooksCommand } from "./commands/hooks";
import { importCommand } from "./commands/import";
import { initCommand } from "./commands/init";
import { lintCommand } from "./commands/lint";
import { linkCommand } from "./commands/link";
import { mcpToolsCommand } from "./commands/mcp-tools";
import { planCommand } from "./commands/plan";
import { promptCommand } from "./commands/prompt";
import { recapCommand } from "./commands/recap";
import { listCommand } from "./commands/list";
import { removeCommand } from "./commands/remove";
import { searchCommand } from "./commands/search";
import { seedConventionsCommand } from "./commands/seed-conventions";
import { statsCommand } from "./commands/stats";
import { statusCommand } from "./commands/status";
import { suggestCommand } from "./commands/suggest";
import { syncCommand } from "./commands/sync";
import { syncClaudeMdCommand } from "./commands/sync-claude-md";
import { tagsCommand } from "./commands/tags";
import { taskCommand } from "./commands/task";
import { unlinkCommand } from "./commands/unlink";
import { updateCommand } from "./commands/update";
import { whyCommand } from "./commands/why";
import { generateZshCompletions } from "./lib/completions";

const program = new Command();

program
    .name("fubbik")
    .description("A local-first knowledge framework for humans and machines")
    .version("0.0.1")
    .option("--json", "output as JSON (machine-readable)")
    .option("-q, --quiet", "minimal output (just IDs/values)");

program.addCommand(initCommand);
program.addCommand(healthCommand);
program.addCommand(addCommand);
program.addCommand(catCommand);
program.addCommand(bulkAddCommand);
program.addCommand(getCommand);
program.addCommand(listCommand);
program.addCommand(searchCommand);
program.addCommand(updateCommand);
program.addCommand(removeCommand);
program.addCommand(linkCommand);
program.addCommand(unlinkCommand);
program.addCommand(tagsCommand);
program.addCommand(statsCommand);
program.addCommand(statusCommand);
program.addCommand(exportCommand);
program.addCommand(exportSiteCommand);
program.addCommand(importCommand);
program.addCommand(diffCommand);
program.addCommand(syncCommand);
program.addCommand(syncClaudeMdCommand);
program.addCommand(enrichCommand);
program.addCommand(codebaseCommand);
program.addCommand(contextCommand);
program.addCommand(contextDirCommand);
program.addCommand(contextForCommand);
program.addCommand(generateCommand);
program.addCommand(requirementsCommand);
program.addCommand(checkFilesCommand);
program.addCommand(hooksCommand);
program.addCommand(lintCommand);
program.addCommand(planCommand);
program.addCommand(promptCommand);
program.addCommand(recapCommand);
program.addCommand(suggestCommand);
program.addCommand(taskCommand);
program.addCommand(doctorCommand);
program.addCommand(mcpToolsCommand);
program.addCommand(importRequirementsCommand);
program.addCommand(kbDiffCommand);
program.addCommand(watchCommand);
program.addCommand(whyCommand);
program.addCommand(gapsCommand);
program.addCommand(seedConventionsCommand);

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
