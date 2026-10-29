import { Command } from "commander";

import { addCommand } from "./add";
import { bulkAddCommand } from "./bulk-add";
import { catCommand } from "./cat";
import { enrichCommand } from "./enrich";
import { getCommand } from "./get";
import { linkCommand } from "./link";
import { listCommand } from "./list";
import { quickCommand } from "./quick";
import { removeCommand } from "./remove";
import { searchCommand } from "./search";
import { unlinkCommand } from "./unlink";
import { updateCommand } from "./update";

export const chunkCommand = new Command("chunk")
    .description("Manage knowledge chunks")
    .addCommand(addCommand)
    .addCommand(bulkAddCommand)
    .addCommand(catCommand)
    .addCommand(enrichCommand)
    .addCommand(getCommand)
    .addCommand(linkCommand)
    .addCommand(listCommand)
    .addCommand(quickCommand)
    .addCommand(removeCommand)
    .addCommand(searchCommand)
    .addCommand(unlinkCommand)
    .addCommand(updateCommand);
