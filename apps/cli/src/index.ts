import { Command } from "commander";

import { addCommand } from "./commands/add";
import { bulkAddCommand } from "./commands/bulk-add";
import { catCommand } from "./commands/cat";
import { codebaseCommand } from "./commands/codebase";
import { contextCommand } from "./commands/context";
import { diffCommand } from "./commands/diff";
import { enrichCommand } from "./commands/enrich";
import { exportCommand } from "./commands/export";
import { generateCommand } from "./commands/generate";
import { requirementsCommand } from "./commands/requirements";
import { getCommand } from "./commands/get";
import { healthCommand } from "./commands/health";
import { importCommand } from "./commands/import";
import { initCommand } from "./commands/init";
import { linkCommand } from "./commands/link";
import { listCommand } from "./commands/list";
import { removeCommand } from "./commands/remove";
import { searchCommand } from "./commands/search";
import { statsCommand } from "./commands/stats";
import { syncCommand } from "./commands/sync";
import { tagsCommand } from "./commands/tags";
import { unlinkCommand } from "./commands/unlink";
import { updateCommand } from "./commands/update";

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
program.addCommand(exportCommand);
program.addCommand(importCommand);
program.addCommand(diffCommand);
program.addCommand(syncCommand);
program.addCommand(enrichCommand);
program.addCommand(codebaseCommand);
program.addCommand(contextCommand);
program.addCommand(generateCommand);
program.addCommand(requirementsCommand);

program.parse();
