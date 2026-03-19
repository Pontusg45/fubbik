import * as vscode from "vscode";
import type { FubbikApi } from "./api";

export class FubbikStatusBar {
    private item: vscode.StatusBarItem;

    constructor() {
        this.item = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Left,
            100
        );
        this.item.command = "fubbik.sidebar.focus";
        this.item.tooltip = "Fubbik Knowledge Base";
    }

    async update(api: FubbikApi, codebaseId?: string) {
        try {
            const res = await api.getChunks(codebaseId);
            this.item.text = `$(book) ${res.total} chunks`;
            this.item.color = undefined;
            this.item.tooltip = "Fubbik Knowledge Base";
            this.item.show();
        } catch {
            this.item.text = "$(book) Fubbik";
            this.item.color = new vscode.ThemeColor("errorForeground");
            this.item.tooltip = "Cannot connect to Fubbik server";
            this.item.show();
        }
    }

    dispose() {
        this.item.dispose();
    }
}
