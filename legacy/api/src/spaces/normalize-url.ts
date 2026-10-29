export function normalizeGitUrl(url: string): string {
    let normalized = url.trim();
    const sshMatch = normalized.match(/^(?:ssh:\/\/)?git@([^:/]+)[:/](.+)$/);
    if (sshMatch) {
        normalized = `${sshMatch[1]}/${sshMatch[2]}`;
    } else {
        normalized = normalized.replace(/^[a-z+]+:\/\//, "");
        normalized = normalized.replace(/^[^@]+@/, "");
    }
    normalized = normalized.replace(/\.git$/, "");
    normalized = normalized.replace(/\/+$/, "");
    return normalized;
}
