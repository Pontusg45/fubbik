import { Command } from "commander";

import { importRequirementsCommand } from "./import-requirements";
import { requirementsCommand } from "./requirements";

export const reqCommand = new Command("req")
    .description("Manage requirements");

// Hoist subcommands from requirementsCommand to avoid double nesting (fubbik req requirements list)
for (const sub of requirementsCommand.commands) {
    reqCommand.addCommand(sub);
}

reqCommand.addCommand(importRequirementsCommand);
