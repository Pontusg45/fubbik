import * as fs from "fs";
import * as path from "path";

import * as vscode from "vscode";

import { FubbikApi } from "./api";
import { registerCreateChunkCommand } from "./create-chunk";
import { detectCodebase } from "./detect-codebase";
import { getChunksForFile } from "./file-chunks";
import { SidebarProvider } from "./sidebar-provider";
import { FubbikStatusBar } from "./status-bar";

function loadProjectConfig(): { serverUrl?: string; webAppUrl?: string; codebase?: string } {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    if (!workspaceFolder) return {};

    const configFiles = ["fubbik.config.json", ".fubbikrc.json"];
    for (const name of configFiles) {
        const configPath = path.join(workspaceFolder.uri.fsPath, name);
        if (fs.existsSync(configPath)) {
            try {
                return JSON.parse(fs.readFileSync(configPath, "utf-8"));
            } catch {
                /* ignore parse errors */
            }
        }
    }
    return {};
}

let codebaseId: string | null = null;

export async function activate(context: vscode.ExtensionContext) {
    const vsCodeConfig = vscode.workspace.getConfiguration("fubbik");
    const projectConfig = loadProjectConfig();
    const serverUrl = vsCodeConfig.get<string>("serverUrl") || projectConfig.serverUrl || "http://localhost:3000";
    const webAppUrl = vsCodeConfig.get<string>("webAppUrl") || projectConfig.webAppUrl || "http://localhost:3001";
    const api = new FubbikApi(serverUrl);
    const sidebarProvider = new SidebarProvider(context.extensionUri);
    sidebarProvider.setApi(api);

    // Status bar
    const statusBar = new FubbikStatusBar();
    context.subscriptions.push(statusBar);

    // Register sidebar
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(SidebarProvider.viewType, sidebarProvider)
    );

    // Refresh command
    async function refreshChunks() {
        sidebarProvider.setState({ loading: true, error: null });
        try {
            const [chunksRes, tags] = await Promise.all([
                api.getChunks(codebaseId ?? undefined),
                api.getTags(),
            ]);
            sidebarProvider.setState({ chunks: chunksRes.chunks, total: chunksRes.total, tags, loading: false });
            statusBar.update(api, codebaseId ?? undefined);
        } catch {
            sidebarProvider.setState({
                chunks: [],
                total: 0,
                loading: false,
                error: `Cannot connect to ${serverUrl}`
            });
            statusBar.update(api, codebaseId ?? undefined);
        }
    }

    context.subscriptions.push(
        vscode.commands.registerCommand("fubbik.refreshSidebar", refreshChunks)
    );

    // Create chunk command
    context.subscriptions.push(
        registerCreateChunkCommand(context, api, () => codebaseId, refreshChunks)
    );

    // Search chunks command
    context.subscriptions.push(
        vscode.commands.registerCommand("fubbik.searchChunks", async () => {
            const quickPick = vscode.window.createQuickPick();
            quickPick.placeholder = "Search chunks...";
            quickPick.matchOnDescription = true;

            quickPick.onDidChangeValue(async (value) => {
                if (value.length < 2) {
                    quickPick.items = [];
                    return;
                }
                quickPick.busy = true;
                try {
                    const result = await api.searchChunks(value, codebaseId ?? undefined);
                    quickPick.items = result.chunks.map((chunk) => ({
                        label: chunk.title || "Untitled",
                        description: `${chunk.source || "note"} ${(chunk.tags || []).join(", ")}`,
                        detail: chunk.content?.substring(0, 100),
                        id: chunk.id,
                    }));
                } catch {
                    quickPick.items = [];
                }
                quickPick.busy = false;
            });

            quickPick.onDidAccept(() => {
                const selected = quickPick.selectedItems[0] as { id?: string } | undefined;
                if (selected?.id) {
                    import("./chunk-detail").then(({ showChunkDetail }) => {
                        showChunkDetail(api, selected.id!);
                    });
                }
                quickPick.dispose();
            });

            quickPick.onDidHide(() => quickPick.dispose());
            quickPick.show();
        })
    );

    // Open graph in browser
    context.subscriptions.push(
        vscode.commands.registerCommand("fubbik.openGraph", () => {
            vscode.env.openExternal(vscode.Uri.parse(`${webAppUrl}/graph`));
        })
    );

    // Open dashboard in browser
    context.subscriptions.push(
        vscode.commands.registerCommand("fubbik.openDashboard", () => {
            vscode.env.openExternal(vscode.Uri.parse(`${webAppUrl}/dashboard`));
        })
    );

    // Quick add note
    context.subscriptions.push(
        vscode.commands.registerCommand("fubbik.quickAddNote", async () => {
            const title = await vscode.window.showInputBox({
                prompt: "Note title",
                placeHolder: "Enter a title for the note",
            });
            if (!title) return;

            const content = await vscode.window.showInputBox({
                prompt: "Note content",
                placeHolder: "Enter note content",
            });
            if (!content) return;

            try {
                const body: Parameters<typeof api.createChunk>[0] = {
                    content,
                    title,
                    source: "note",
                };
                if (codebaseId) body.codebaseId = codebaseId;
                await api.createChunk(body);
                vscode.window.showInformationMessage("Note added to Fubbik!");
                vscode.commands.executeCommand("fubbik.refreshSidebar");
            } catch (err) {
                const message = err instanceof Error ? err.message : "Unknown error";
                vscode.window.showErrorMessage(`Failed to add note: ${message}`);
            }
        })
    );

    // File-aware chunk surfacing
    context.subscriptions.push(
        vscode.window.onDidChangeActiveTextEditor(async (editor) => {
            if (editor) {
                try {
                    const fileChunks = await getChunksForFile(api, editor.document.uri);
                    sidebarProvider.setFileChunks(fileChunks);
                } catch {
                    sidebarProvider.setFileChunks([]);
                }
            } else {
                sidebarProvider.setFileChunks([]);
            }
        })
    );

    // Detect codebase on activation
    try {
        const result = await detectCodebase(api);
        if (result) {
            codebaseId = result.id;
            sidebarProvider.setState({ codebaseName: result.name });
        } else {
            sidebarProvider.setState({ codebaseName: null });
        }
    } catch {
        sidebarProvider.setState({ codebaseName: null });
    }

    // Fetch initial chunks
    await refreshChunks();

    // Check file chunks for current editor
    if (vscode.window.activeTextEditor) {
        try {
            const fileChunks = await getChunksForFile(api, vscode.window.activeTextEditor.document.uri);
            sidebarProvider.setFileChunks(fileChunks);
        } catch {
            // Ignore errors on initial load
        }
    }

    // Re-detect on workspace folder change
    context.subscriptions.push(
        vscode.workspace.onDidChangeWorkspaceFolders(async () => {
            try {
                const result = await detectCodebase(api);
                codebaseId = result?.id ?? null;
                sidebarProvider.setState({ codebaseName: result?.name ?? null });
            } catch {
                codebaseId = null;
                sidebarProvider.setState({ codebaseName: null });
            }
            await refreshChunks();
        })
    );
}

export function deactivate() {}
