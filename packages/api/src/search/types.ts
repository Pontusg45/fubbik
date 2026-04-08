export interface QueryClause {
    field: string;
    operator: string;
    value: string;
    params?: Record<string, string>;
    negate?: boolean;
}

export interface SearchQuery {
    clauses: QueryClause[];
    join: "and" | "or";
    sort?: "relevance" | "newest" | "oldest" | "updated";
    limit?: number;
    offset?: number;
    codebaseId?: string;
}

export interface GraphContext {
    hopDistance?: number;
    pathPosition?: number;
    matchedRequirement?: string;
}

export interface SearchResultChunk {
    id: string;
    title: string;
    type: string;
    summary: string | null;
    tags: string[];
    connectionCount: number;
    updatedAt: Date;
    graphContext?: GraphContext;
}

export interface SearchResult {
    chunks: SearchResultChunk[];
    total: number;
    graphMeta?: {
        type: "neighborhood" | "path" | "requirement-reach";
        referenceChunk?: string;
        pathChunks?: string[];
    };
}
