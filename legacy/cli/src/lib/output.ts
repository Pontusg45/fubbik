import type { Command } from "commander";

import { formatError } from "./colors";

interface GlobalOpts {
    json?: boolean;
    quiet?: boolean;
}

function getGlobalOpts(cmd: Command): GlobalOpts {
    const root = cmd.parent ?? cmd;
    return root.opts() as GlobalOpts;
}

/** Print structured data. In --json mode: JSON. In --quiet mode: nothing. Otherwise: human text. */
export function output(cmd: Command, data: unknown, humanText: string): void {
    const opts = getGlobalOpts(cmd);
    if (opts.json) {
        console.log(JSON.stringify(data, null, 2));
    } else if (!opts.quiet) {
        console.log(humanText);
    }
}

/** Print just the essential value (e.g. an ID). Prints in all modes except --json (where output() handles it). */
export function outputQuiet(cmd: Command, value: string): void {
    const opts = getGlobalOpts(cmd);
    if (opts.quiet && !opts.json) {
        console.log(value);
    }
}

/** Print an error. Always prints to stderr regardless of flags. */
export function outputError(message: string): void {
    console.error(formatError(message));
}

export function isJson(cmd: Command): boolean {
    return getGlobalOpts(cmd).json === true;
}

export function isQuiet(cmd: Command): boolean {
    return getGlobalOpts(cmd).quiet === true;
}
