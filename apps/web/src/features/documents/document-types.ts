/* ─── Document Browser Types ─── */

export interface DocumentListItem {
    id: string;
    title: string;
    sourcePath: string;
    description: string | null;
    chunkCount: number;
    updatedAt: Date;
    lastChunkUpdatedAt: Date | null;
    oldestChunkUpdatedAt: Date | null;
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
