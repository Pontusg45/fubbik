/* ─── Document Browser Types ─── */

export interface DocumentListItem {
    id: string;
    title: string;
    sourcePath: string;
    description: string | null;
    chunkCount: number;
    // ISO date-time strings over the wire (both Node and Rust) — these were
    // incorrectly typed as `Date` even under `legacyApi`/Eden; fixed while
    // moving this call site to `api` since openapi-typescript's precise
    // string types caught what Eden's looser inference let through.
    updatedAt: string;
    lastChunkUpdatedAt: string | null;
    oldestChunkUpdatedAt: string | null;
    tags: string[];
    type: string;
}

export interface DocumentChunk {
    id: string;
    title: string;
    content: string;
    documentOrder: number | null;
}

export interface DocumentDetail {
    id: string;
    title: string;
    sourcePath: string;
    description: string | null;
    chunks: DocumentChunk[];
}

export interface SearchResult {
    documentId: string;
    documentTitle: string;
    sourcePath: string;
    chunk: DocumentChunk;
    snippet: string;
}

export interface DocumentBrowserProps {
    initialDocId?: string;
    initialSection?: string;
    initialGroupBy?: "folder" | "tag";
    initialTags?: string[];
    initialTypes?: string[];
}
