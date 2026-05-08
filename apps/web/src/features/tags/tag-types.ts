export interface Tag {
    id: string;
    name: string;
    tagTypeId: string | null;
    tagTypeName: string | null;
    tagTypeColor: string | null;
    chunkCount: number;
}

export interface TagType {
    id: string;
    name: string;
    color: string;
}

export type SortMode = "alpha" | "usage";
