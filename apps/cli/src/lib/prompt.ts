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
