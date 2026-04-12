import { Command } from "commander";

import { cleanupCommand } from "./cleanup";
import { doctorCommand } from "./doctor";
import { healthCommand } from "./health";
import { lintCommand } from "./lint";
import { seedConventionsCommand } from "./seed-conventions";

export const maintainCommand = new Command("maintain")
    .description("Maintenance and diagnostics")
    .addCommand(cleanupCommand)
    .addCommand(doctorCommand)
    .addCommand(healthCommand)
    .addCommand(lintCommand)
    .addCommand(seedConventionsCommand);
