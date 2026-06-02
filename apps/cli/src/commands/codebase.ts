import { Command } from "commander";

import { spaceCommand } from "./space";

// Legacy alias — `fubbik codebase ...` forwards to `fubbik space ...`.
// Remove after one release.
export const codebaseCommand = new Command("codebase")
    .description("[deprecated] alias of `fubbik space`")
    .hook("preAction", () => {
        console.warn("[fubbik] `codebase` is deprecated; use `space` instead.");
    });

for (const sub of spaceCommand.commands) {
    codebaseCommand.addCommand(sub);
}
