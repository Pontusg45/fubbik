export interface ApplyToRow {
    pattern: string;
    note: string;
}

export interface FileRefRow {
    path: string;
    anchor: string;
    relation: "documents" | "configures" | "tests" | "implements";
}

export function isValidGlob(pattern: string): boolean {
    if (!pattern.trim()) return true;
    const unmatched = (pattern.match(/\[/g) || []).length !== (pattern.match(/\]/g) || []).length;
    const emptyBraces = /\{\s*\}/.test(pattern);
    return !unmatched && !emptyBraces;
}

export function validateChunkContent(title: string, content: string): Record<string, string> {
    const errors: Record<string, string> = {};
    if (!title.trim()) errors.title = "Title is required";
    else if (title.length > 200) errors.title = "Title must be 200 characters or less";
    if (content.length > 50000) errors.content = "Content must be 50,000 characters or less";
    return errors;
}
