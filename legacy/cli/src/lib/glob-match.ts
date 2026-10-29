export function globMatch(pattern: string, path: string): boolean {
    const regexStr = pattern
        .replace(/\./g, "\\.")
        .replace(/\*\*/g, "{{GLOBSTAR}}")
        .replace(/\*/g, "[^/]*")
        .replace(/\?/g, "[^/]")
        .replace(/\{\{GLOBSTAR\}\}/g, ".*");
    return new RegExp(`^${regexStr}$`).test(path);
}
