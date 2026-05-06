export interface EnrichedDocument {
    id: string;
    title: string;
    sourcePath: string;
    tags: string[];
    type: string;
    chunkCount: number;
}

export interface DocFilters {
    activeTags: string[];
    activeTypes: string[];
}

export function filterDocuments(
    documents: EnrichedDocument[],
    filters: DocFilters
): EnrichedDocument[] {
    const { activeTags, activeTypes } = filters;

    return documents.filter(doc => {
        if (activeTags.length > 0) {
            const hasTag = activeTags.some(t => doc.tags.includes(t));
            if (!hasTag) return false;
        }
        if (activeTypes.length > 0) {
            const hasType = activeTypes.includes(doc.type);
            if (!hasType) return false;
        }
        return true;
    });
}

function folderFromPath(sourcePath: string): string {
    const parts = sourcePath.split("/");
    if (parts.length <= 1) return "/";
    return parts.slice(0, -1).join("/");
}

export function groupDocuments(
    documents: EnrichedDocument[],
    groupBy: "folder" | "tag"
): Map<string, EnrichedDocument[]> {
    const groups = new Map<string, EnrichedDocument[]>();

    if (groupBy === "folder") {
        for (const doc of documents) {
            const folder = folderFromPath(doc.sourcePath);
            const existing = groups.get(folder) ?? [];
            existing.push(doc);
            groups.set(folder, existing);
        }
    } else {
        for (const doc of documents) {
            if (doc.tags.length === 0) {
                const existing = groups.get("Untagged") ?? [];
                existing.push(doc);
                groups.set("Untagged", existing);
            } else {
                for (const tag of doc.tags) {
                    const existing = groups.get(tag) ?? [];
                    existing.push(doc);
                    groups.set(tag, existing);
                }
            }
        }
    }

    return new Map([...groups.entries()].sort((a, b) => {
        if (a[0] === "Untagged") return 1;
        if (b[0] === "Untagged") return -1;
        return a[0].localeCompare(b[0]);
    }));
}

export function collectAllTags(documents: EnrichedDocument[]): string[] {
    const tags = new Set<string>();
    for (const doc of documents) {
        for (const tag of doc.tags) tags.add(tag);
    }
    return [...tags].sort();
}

export function collectAllTypes(documents: EnrichedDocument[]): string[] {
    const types = new Set<string>();
    for (const doc of documents) types.add(doc.type);
    return [...types].sort();
}
