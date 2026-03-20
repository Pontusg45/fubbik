import {
    chmodSync,
    existsSync,
    readFileSync,
    unlinkSync,
    writeFileSync,
} from "node:fs";
import { execSync } from "node:child_process";
import { join } from "node:path";

import { Command } from "commander";

import { formatError, formatSuccess } from "../lib/colors";

const HOOK_SCRIPT = `#!/bin/sh
# Installed by fubbik — chunk-aware pre-commit hook
fubbik check-files --staged 2>&1 || true
`;

function getGitRoot(): string {
    return execSync("git rev-parse --show-toplevel", {
        encoding: "utf-8",
    }).trim();
}

function hookPath(): string {
    const root = getGitRoot();
    return join(root, ".git", "hooks", "pre-commit");
}

export const hooksCommand = new Command("hooks").description(
    "Manage git hooks for chunk-aware commits"
);

hooksCommand
    .command("install")
    .description("Install a pre-commit hook that checks staged files against chunks")
    .option("--force", "overwrite existing pre-commit hook")
    .action((opts: { force?: boolean }) => {
        const path = hookPath();

        if (existsSync(path) && !opts.force) {
            console.error(
                formatError(
                    "A pre-commit hook already exists. Use --force to overwrite."
                )
            );
            process.exit(1);
        }

        writeFileSync(path, HOOK_SCRIPT);
        chmodSync(path, 0o755);
        console.log(formatSuccess("Installed fubbik pre-commit hook."));
    });

hooksCommand
    .command("uninstall")
    .description("Remove the fubbik pre-commit hook")
    .action(() => {
        const path = hookPath();

        if (!existsSync(path)) {
            console.error(formatError("No pre-commit hook found."));
            process.exit(1);
        }

        const content = readFileSync(path, "utf-8");
        if (!content.includes("fubbik")) {
            console.error(
                formatError(
                    "Pre-commit hook was not installed by fubbik. Refusing to remove."
                )
            );
            process.exit(1);
        }

        unlinkSync(path);
        console.log(formatSuccess("Removed fubbik pre-commit hook."));
    });
