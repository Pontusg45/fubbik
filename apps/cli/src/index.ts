import { Command } from "commander";

import { addCommand } from "./commands/add";
import { getCommand } from "./commands/get";
import { healthCommand } from "./commands/health";
import { initCommand } from "./commands/init";
import { listCommand } from "./commands/list";
import { removeCommand } from "./commands/remove";
import { searchCommand } from "./commands/search";
import { syncCommand } from "./commands/sync";
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
program.addCommand(getCommand);
program.addCommand(listCommand);
program.addCommand(searchCommand);
program.addCommand(updateCommand);
program.addCommand(removeCommand);
program.addCommand(syncCommand);

program.parse();
