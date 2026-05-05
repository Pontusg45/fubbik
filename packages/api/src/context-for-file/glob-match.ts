export function normalizePath(path: string): string {
    return path
        .replace(/^\.\//, "")   // strip leading ./
        .replace(/^\//, "")     // strip leading /
        .replace(/\/+/g, "/")   // collapse consecutive /
        .replace(/\/$/, "");    // strip trailing /
}

export function globMatch(pattern: string, path: string): boolean {
    const normalizedPattern = normalizePath(pattern);
    const normalizedPath = normalizePath(path);

    const regexStr = normalizedPattern
        .replace(/\./g, "\\.")
        .replace(/\*\*/g, "{{GLOBSTAR}}")
        .replace(/\*/g, "[^/]*")
        .replace(/\?/g, "[^/]")
        .replace(/\{\{GLOBSTAR\}\}/g, ".*");
    return new RegExp(`^${regexStr}$`).test(normalizedPath);
}
