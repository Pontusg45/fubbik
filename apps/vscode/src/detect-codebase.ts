import * as vscode from "vscode";
import { exec } from "child_process";
import { FubbikApi, DetectResult } from "./api";

function execAsync(
    command: string,
    cwd: string
): Promise<{ stdout: string; stderr: string }> {
    return new Promise((resolve, reject) => {
        exec(command, { cwd }, (error, stdout, stderr) => {
            if (error) {
                reject(error);
            } else {
                resolve({ stdout, stderr });
            }
        });
    });
}

export async function detectCodebase(
    api: FubbikApi
): Promise<DetectResult | null> {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) {
        return null;
    }

    const workspacePath = folders[0].uri.fsPath;

    try {
        const { stdout } = await execAsync(
            "git remote get-url origin",
            workspacePath
        );
        const remoteUrl = stdout.trim();

        if (remoteUrl) {
            return api.detectCodebase({ remoteUrl });
        }
    } catch {
        // No git remote — fall through to local path detection
    }

    return api.detectCodebase({ localPath: workspacePath });
}
