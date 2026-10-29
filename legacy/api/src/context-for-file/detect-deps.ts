export function parseDependencies(filename: string, content: string): string[] {
    if (filename === "package.json") {
        try {
            const pkg = JSON.parse(content);
            return Object.keys(pkg.dependencies ?? {});
        } catch {
            return [];
        }
    }
    if (filename === "go.mod") {
        const deps: string[] = [];
        const requireBlock = content.match(/require\s*\(([\s\S]*?)\)/);
        if (requireBlock) {
            for (const line of requireBlock[1]!.split("\n")) {
                const match = line.trim().match(/^(\S+)\s+/);
                if (match) deps.push(match[1]!);
            }
        }
        return deps;
    }
    return [];
}
