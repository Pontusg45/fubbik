import * as vscode from "vscode";

import { FubbikApi } from "./api";
import { registerCreateChunkCommand } from "./create-chunk";
import { detectCodebase } from "./detect-codebase";
import { SidebarProvider } from "./sidebar-provider";

let codebaseId: string | null = null;

export async function activate(context: vscode.ExtensionContext) {
    const config = vscode.workspace.getConfiguration("fubbik");
    const serverUrl = config.get<string>("serverUrl", "http://localhost:3000");
    const api = new FubbikApi(serverUrl);
    const sidebarProvider = new SidebarProvider(context.extensionUri);
    sidebarProvider.setApi(api);

    // Register sidebar
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(SidebarProvider.viewType, sidebarProvider)
    );

    // Refresh command
    async function refreshChunks() {
        sidebarProvider.setState({ loading: true, error: null });
        try {
            const { chunks, total } = await api.getChunks(codebaseId ?? undefined);
            sidebarProvider.setState({ chunks, total, loading: false });
        } catch {
            sidebarProvider.setState({
                chunks: [],
                total: 0,
                loading: false,
                error: `Cannot connect to ${serverUrl}`
            });
        }
    }

    context.subscriptions.push(
        vscode.commands.registerCommand("fubbik.refreshSidebar", refreshChunks)
    );

    // Create chunk command
    context.subscriptions.push(
        registerCreateChunkCommand(context, api, () => codebaseId, refreshChunks)
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
