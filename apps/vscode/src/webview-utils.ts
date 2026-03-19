import * as vscode from "vscode";
import * as crypto from "crypto";

export function getNonce(): string {
    return crypto.randomBytes(16).toString("hex");
}

export function getBaseHtml(
    webview: vscode.Webview,
    nonce: string,
    body: string,
    script: string
): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta
        http-equiv="Content-Security-Policy"
        content="default-src 'none'; style-src 'nonce-${nonce}'; script-src 'nonce-${nonce}';"
    />
    <style nonce="${nonce}">
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: var(--vscode-font-family);
            font-size: var(--vscode-font-size);
            color: var(--vscode-foreground);
            background-color: var(--vscode-editor-background);
            padding: 12px;
        }

        input,
        textarea,
        select {
            background-color: var(--vscode-input-background);
            color: var(--vscode-input-foreground);
            border: 1px solid var(--vscode-input-border);
            border-radius: 2px;
            padding: 4px 8px;
            font-family: inherit;
            font-size: inherit;
            width: 100%;
        }

        input:focus,
        textarea:focus,
        select:focus {
            outline: 1px solid var(--vscode-focusBorder);
            border-color: var(--vscode-focusBorder);
        }

        button {
            background-color: var(--vscode-button-background);
            color: var(--vscode-button-foreground);
            border: none;
            border-radius: 2px;
            padding: 6px 14px;
            font-family: inherit;
            font-size: inherit;
            cursor: pointer;
        }

        button:hover {
            background-color: var(--vscode-button-hoverBackground);
        }

        button.secondary {
            background-color: var(--vscode-button-secondaryBackground);
            color: var(--vscode-button-secondaryForeground);
        }

        button.secondary:hover {
            background-color: var(--vscode-button-secondaryHoverBackground);
        }

        .badge {
            display: inline-block;
            background-color: var(--vscode-badge-background);
            color: var(--vscode-badge-foreground);
            border-radius: 10px;
            padding: 2px 8px;
            font-size: 0.85em;
        }

        .list-item {
            padding: 8px;
            border-bottom: 1px solid var(--vscode-panel-border);
            cursor: pointer;
        }

        .list-item:hover {
            background-color: var(--vscode-list-hoverBackground);
        }

        .list-item.active {
            background-color: var(--vscode-list-activeSelectionBackground);
            color: var(--vscode-list-activeSelectionForeground);
        }

        .muted {
            color: var(--vscode-descriptionForeground);
        }

        .error {
            color: var(--vscode-errorForeground);
        }

        a {
            color: var(--vscode-textLink-foreground);
            text-decoration: none;
        }

        a:hover {
            color: var(--vscode-textLink-activeForeground);
            text-decoration: underline;
        }

        h2, h3 {
            font-weight: 600;
            margin-bottom: 8px;
        }

        .mb-1 { margin-bottom: 4px; }
        .mb-2 { margin-bottom: 8px; }
        .mb-3 { margin-bottom: 12px; }
        .mt-2 { margin-top: 8px; }
        .mt-3 { margin-top: 12px; }

        .filters {
            display: flex;
            flex-direction: column;
            gap: 4px;
        }

        .filter-select {
            width: 100%;
            padding: 3px 6px;
            font-size: 0.9em;
        }

        .tag-badge {
            display: inline-block;
            background-color: var(--vscode-badge-background);
            color: var(--vscode-badge-foreground);
            border-radius: 8px;
            padding: 1px 6px;
            font-size: 0.8em;
            opacity: 0.8;
        }
    </style>
</head>
<body>
    ${body}
    <script nonce="${nonce}">
        ${script}
    </script>
</body>
</html>`;
}
