export interface DiscoveredChunk {
    title: string;
    content: string;
    type: string;
    tags: string[];
    tier: 1 | 2 | 3;
    /** Category for preview grouping: documents, tech-stack, structure, conventions, config */
    category: "documents" | "tech-stack" | "structure" | "conventions" | "config";
    /** File path or detection name that produced this chunk */
    source: string;
    /** Glob patterns for file-area linking */
    appliesTo?: string[];
}

export interface DiscoveredConnection {
    /** Title of the source chunk (resolved to ID after import) */
    sourceTitle: string;
    /** Title of the target chunk (resolved to ID after import) */
    targetTitle: string;
    relation: string;
}

export interface Tip {
    title: string;
    detail: string;
}

export interface DiscoveryResult {
    codebase: { name: string; remoteUrl: string | null; localPath: string };
    chunks: DiscoveredChunk[];
    connections: DiscoveredConnection[];
    tags: string[];
    tips: Tip[];
}
