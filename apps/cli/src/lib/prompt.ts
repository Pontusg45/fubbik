import * as readline from "readline";

export function parseConfirmInput(input: string): boolean {
    return ["y", "yes"].includes(input.trim().toLowerCase());
}

export async function confirm(message: string): Promise<boolean> {
    const rl = readline.createInterface({ input: process.stdin, output: process.stderr });
    return new Promise((resolve) => {
        rl.question(`${message} [y/N] `, (answer) => {
            rl.close();
            resolve(parseConfirmInput(answer));
        });
    });
}

export async function promptInput(message: string, defaultValue = ""): Promise<string> {
    const rl = readline.createInterface({ input: process.stdin, output: process.stderr });
    const suffix = defaultValue ? ` (${defaultValue})` : "";
    return new Promise((resolve) => {
        rl.question(`${message}${suffix}: `, (answer) => {
            rl.close();
            resolve(answer.trim() || defaultValue);
        });
    });
}

export async function openEditor(initialContent = ""): Promise<string> {
    const { execFileSync } = await import("child_process");
    const { writeFileSync, readFileSync, unlinkSync } = await import("fs");
    const { tmpdir } = await import("os");
    const { join } = await import("path");

    const tmpFile = join(tmpdir(), `fubbik-${Date.now()}.md`);
    writeFileSync(tmpFile, initialContent);

    const editor = process.env.EDITOR || process.env.VISUAL || "vi";
    const parts = editor.split(" ");
    const cmd = parts[0]!;
    const args = parts.slice(1);
    execFileSync(cmd, [...args, tmpFile], { stdio: "inherit" });

    const content = readFileSync(tmpFile, "utf-8");
    unlinkSync(tmpFile);
    return content;
}
