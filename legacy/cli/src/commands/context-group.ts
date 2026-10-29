import { Command } from "commander";

import { contextCommand } from "./context";
import { contextAboutCommand } from "./context-about";
import { contextDirCommand } from "./context-dir";
import { contextForCommand } from "./context-for";
import { contextForDiffCommand } from "./context-for-diff";
import { contextForPlanCommand } from "./context-for-plan";
import { contextSnapshotCommand } from "./context-snapshot";

export const contextGroupCommand = new Command("context")
    .description("Export context for AI consumption")
    .addCommand(contextCommand)
    .addCommand(contextDirCommand)
    .addCommand(contextForCommand)
    .addCommand(contextForPlanCommand)
    .addCommand(contextAboutCommand)
    .addCommand(contextForDiffCommand)
    .addCommand(contextSnapshotCommand);
