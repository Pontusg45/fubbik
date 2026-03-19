import * as vscode from "vscode";
import type { Chunk, FubbikApi } from "./api";
import { getBaseHtml, getNonce } from "./webview-utils";

interface SidebarState {
    codebaseName?: string;
    chunks?: Chunk[];
    total?: number;
    tags?: Array<{ id: string; name: string }>;
    error?: string;
    loading?: boolean;
    fileChunks?: Chunk[];
}

export class SidebarProvider implements vscode.WebviewViewProvider {
    public static readonly viewType = "fubbik.sidebar";

    private view?: vscode.WebviewView;
    private state: SidebarState = {};

    private api?: FubbikApi;

    constructor(private readonly extensionUri: vscode.Uri) {}

    public setApi(api: FubbikApi) {
        this.api = api;
    }

    public resolveWebviewView(
        webviewView: vscode.WebviewView,
        _context: vscode.WebviewViewResolveContext,
        _token: vscode.CancellationToken
    ): void {
        this.view = webviewView;

        webviewView.webview.options = {
            enableScripts: true,
        };

        webviewView.webview.onDidReceiveMessage(async (message) => {
            switch (message.type) {
                case "openChunk": {
                    if (this.api) {
                        const { showChunkDetail } = await import("./chunk-detail");
                        showChunkDetail(this.api, message.id);
                    }
                    break;
                }
                case "refresh": {
                    vscode.commands.executeCommand("fubbik.refreshSidebar");
                    break;
                }
            }
        });

        this.render();
    }

    public setFileChunks(chunks: Chunk[]): void {
        this.state = { ...this.state, fileChunks: chunks };
        this.render();
    }

    public setState(state: SidebarState): void {
        this.state = { ...this.state, ...state };
        this.render();
    }

    private render(): void {
        if (!this.view) {
            return;
        }

        const nonce = getNonce();
        const body = this.buildBody();
        const script = this.buildScript();

        this.view.webview.html = getBaseHtml(
            this.view.webview,
            nonce,
            body,
            script
        );
    }

    private buildBody(): string {
        const { codebaseName, chunks, total, tags, error, loading, fileChunks } = this.state;

        let html = `<div id="sidebar">`;

        // Header
        html += `<div style="display:flex;align-items:center;justify-content:space-between;" class="mb-3">`;
        html += `<h2>${codebaseName ? escapeHtml(codebaseName) : "Fubbik"}</h2>`;
        html += `<button id="refreshBtn" title="Refresh" style="padding:4px 8px;font-size:1.1em;">&#8635;</button>`;
        html += `</div>`;

        if (loading) {
            html += `<p class="muted">Loading chunks...</p>`;
            html += `</div>`;
            return html;
        }

        if (error) {
            html += `<p class="error">${escapeHtml(error)}</p>`;
            html += `</div>`;
            return html;
        }

        if (!chunks || chunks.length === 0) {
            html += `<p class="muted">No chunks found</p>`;
            html += `</div>`;
            return html;
        }

        // File-related chunks
        if (fileChunks && fileChunks.length > 0) {
            html += `<div class="mb-3" style="border:1px solid var(--vscode-panel-border);border-radius:4px;padding:8px;">`;
            html += `<h3 style="font-size:0.9em;margin-bottom:6px;">Related to this file</h3>`;
            for (const fc of fileChunks) {
                const fcTitle = fc.title || "Untitled";
                const fcType = fc.source || "note";
                html += `<div class="list-item chunk-item" data-id="${escapeAttr(fc.id)}" data-title="${escapeAttr(fcTitle)}" data-type="${escapeAttr(fcType)}" data-tags="${escapeAttr((fc.tags || []).join(","))}" data-created="${escapeAttr(fc.createdAt)}">`;
                html += `<span class="badge">${escapeHtml(fcType)}</span> `;
                html += `<a href="#" class="chunk-link">${escapeHtml(fcTitle)}</a>`;
                html += `</div>`;
            }
            html += `</div>`;
        }

        // Search input
        html += `<input type="text" id="searchInput" placeholder="Search chunks..." class="mb-2" />`;

        // Filter controls
        html += `<div class="filters mb-2">`;
        html += `<select id="filter-type" class="filter-select">`;
        html += `<option value="">All types</option>`;
        html += `<option value="note">Note</option>`;
        html += `<option value="document">Document</option>`;
        html += `<option value="reference">Reference</option>`;
        html += `<option value="schema">Schema</option>`;
        html += `<option value="checklist">Checklist</option>`;
        html += `</select>`;
        html += `<select id="filter-tag" class="filter-select">`;
        html += `<option value="">All tags</option>`;
        if (tags) {
            for (const tag of tags) {
                html += `<option value="${escapeAttr(tag.name)}">${escapeHtml(tag.name)}</option>`;
            }
        }
        html += `</select>`;
        html += `<select id="sort-by" class="filter-select">`;
        html += `<option value="newest">Newest</option>`;
        html += `<option value="oldest">Oldest</option>`;
        html += `<option value="title">A-Z</option>`;
        html += `</select>`;
        html += `</div>`;

        // Info about truncation
        if (total && total > 100) {
            html += `<p class="muted mb-2" style="font-size:0.9em;">Showing first 100 of ${total} chunks</p>`;
        }

        // Chunk list
        html += `<div id="chunkList">`;
        for (const chunk of chunks) {
            const title = chunk.title || "Untitled";
            const date = new Date(chunk.createdAt).toLocaleDateString();
            const type = chunk.source || "note";
            const chunkTags = (chunk.tags || []).join(",");
            html += `<div class="list-item chunk-item" data-id="${escapeAttr(chunk.id)}" data-title="${escapeAttr(title)}" data-type="${escapeAttr(type)}" data-tags="${escapeAttr(chunkTags)}" data-created="${escapeAttr(chunk.createdAt)}">`;
            html += `<span class="badge">${escapeHtml(type)}</span> `;
            html += `<a href="#" class="chunk-link">${escapeHtml(title)}</a>`;
            if (chunk.tags && chunk.tags.length > 0) {
                html += `<div style="margin-top:2px;">`;
                for (const t of chunk.tags) {
                    html += `<span class="tag-badge">${escapeHtml(t)}</span> `;
                }
                html += `</div>`;
            }
            html += `<div class="muted" style="font-size:0.85em;margin-top:2px;">${date}</div>`;
            html += `</div>`;
        }
        html += `</div>`;

        html += `</div>`;
        return html;
    }

    private buildScript(): string {
        return `
            const vscode = acquireVsCodeApi();

            document.getElementById("refreshBtn")?.addEventListener("click", () => {
                vscode.postMessage({ type: "refresh" });
            });

            function applyFilters() {
                const search = (document.getElementById("searchInput")?.value || "").toLowerCase();
                const type = document.getElementById("filter-type")?.value || "";
                const tag = document.getElementById("filter-tag")?.value || "";
                const items = document.querySelectorAll(".chunk-item");
                items.forEach((item) => {
                    const title = (item.dataset.title || "").toLowerCase();
                    const itemType = item.dataset.type || "";
                    const itemTags = item.dataset.tags || "";
                    const matchSearch = !search || title.includes(search);
                    const matchType = !type || itemType === type;
                    const matchTag = !tag || itemTags.split(",").includes(tag);
                    item.style.display = (matchSearch && matchType && matchTag) ? "" : "none";
                });
            }

            function applySort() {
                const sortBy = document.getElementById("sort-by")?.value || "newest";
                const list = document.getElementById("chunkList");
                if (!list) return;
                const items = Array.from(list.querySelectorAll(".chunk-item"));
                items.sort((a, b) => {
                    if (sortBy === "title") {
                        const titleA = (a.dataset.title || "").toLowerCase();
                        const titleB = (b.dataset.title || "").toLowerCase();
                        return titleA.localeCompare(titleB);
                    }
                    const dateA = new Date(a.dataset.created || 0).getTime();
                    const dateB = new Date(b.dataset.created || 0).getTime();
                    return sortBy === "oldest" ? dateA - dateB : dateB - dateA;
                });
                items.forEach((item) => list.appendChild(item));
                applyFilters();
            }

            document.getElementById("searchInput")?.addEventListener("input", applyFilters);
            document.getElementById("filter-type")?.addEventListener("change", applyFilters);
            document.getElementById("filter-tag")?.addEventListener("change", applyFilters);
            document.getElementById("sort-by")?.addEventListener("change", applySort);

            document.getElementById("chunkList")?.addEventListener("click", (e) => {
                const item = e.target.closest(".chunk-item");
                if (item) {
                    const id = item.getAttribute("data-id");
                    if (id) {
                        vscode.postMessage({ type: "openChunk", id });
                    }
                }
            });
        `;
    }
}

function escapeHtml(text: string): string {
    return text
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}

function escapeAttr(text: string): string {
    return text
        .replace(/&/g, "&amp;")
        .replace(/"/g, "&quot;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;");
}
