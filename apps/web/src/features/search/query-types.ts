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
}

export const FILTER_COLORS: Record<string, string> = {
    type: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    origin: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    review: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    tag: "bg-teal-500/15 border-teal-500/30 text-teal-400",
    near: "bg-amber-500/15 border-amber-500/30 text-amber-400",
    path: "bg-amber-500/15 border-amber-500/30 text-amber-400",
    "affected-by": "bg-amber-500/15 border-amber-500/30 text-amber-400",
    "similar-to": "bg-purple-500/15 border-purple-500/30 text-purple-400",
    text: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    connections: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    updated: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    codebase: "bg-slate-500/15 border-slate-500/30 text-slate-400",
};

export const GRAPH_FIELDS = ["near", "path", "affected-by", "similar-to"];

export const FILTER_CATEGORIES = [
    {
        label: "Basic",
        fields: [
            { field: "type", label: "Type", description: "Chunk type" },
            { field: "tag", label: "Tag", description: "Filter by tag" },
            { field: "text", label: "Text search", description: "Search title and content" },
            { field: "connections", label: "Connections", description: "Minimum connections" },
            { field: "updated", label: "Updated within", description: "Days since update" },
            { field: "origin", label: "Origin", description: "Human or AI" },
            { field: "review", label: "Review status", description: "Draft or approved" },
        ],
    },
    {
        label: "Graph",
        fields: [
            { field: "near", label: "Neighborhood", description: "Chunks within N hops" },
            { field: "path", label: "Path finding", description: "Path between two chunks" },
            { field: "affected-by", label: "Affected by requirement", description: "Requirement reach" },
            { field: "similar-to", label: "Similar to", description: "Find chunks with similar content" },
        ],
    },
];
