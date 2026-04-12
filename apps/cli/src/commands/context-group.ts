import { Command } from "commander";

import { contextCommand } from "./context";
import { contextDirCommand } from "./context-dir";
import { contextForCommand } from "./context-for";

export const contextGroupCommand = new Command("context")
    .description("Export context for AI consumption")
    .addCommand(contextCommand)
    .addCommand(contextDirCommand)
    .addCommand(contextForCommand);
