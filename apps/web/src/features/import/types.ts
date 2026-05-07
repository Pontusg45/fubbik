export interface FileEntry {
    path: string;
    content: string;
}

export interface FileConfig {
    title: string;
    type: string;
    tags: string[];
    templateId: string | null;
    folderTags: string[];
}

export interface ImportFileStatus {
    status: "pending" | "importing" | "created" | "skipped" | "error";
    created?: number;
    error?: string;
}

export interface PreviewFileResult {
    path: string;
    title: string;
    suggestedTemplate: {
        id: string;
        name: string;
        score: number;
        type: string;
        tags: string[];
        extractedFields: Record<string, unknown>;
    } | null;
    parsed: {
        title: string;
        type: string;
        tags: string[];
        content: string;
    };
}

export type WizardStep = 1 | 2 | 3 | 4;

export interface TreeNode {
    name: string;
    path: string;
    isDir: boolean;
    isIndex: boolean;
    children: TreeNode[];
}

export const INDEX_FILE_NAMES = new Set(["index.md", "readme.md", "_index.md"]);
