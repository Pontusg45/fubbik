import * as vscode from "vscode";
import type { Chunk, FubbikApi } from "./api";

export async function getChunksForFile(
    api: FubbikApi,
    uri: vscode.Uri
): Promise<Chunk[]> {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    if (!workspaceFolder) return [];
    const relativePath = vscode.workspace.asRelativePath(uri);
    return api.getFileRefLookup(relativePath);
}
