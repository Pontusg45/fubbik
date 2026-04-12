import { Command } from "commander";

import { tagNormalizeCommand } from "./tag-normalize";
import { tagsCommand } from "./tags";

export const tagGroupCommand = new Command("tag")
    .description("Manage tags and tag types")
    .addCommand(tagsCommand)
    .addCommand(tagNormalizeCommand);
