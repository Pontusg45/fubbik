/** Shared presentation thresholds; runtime behavior remains server-owned. */
export interface ChunkSizeThresholds {
    goodLines: number;
    moderateLines: number;
    warningLines: number;
    goodChars: number;
    moderateChars: number;
    warningChars: number;
}

export const DEFAULT_THRESHOLDS: ChunkSizeThresholds = {
    goodLines: 300,
    moderateLines: 600,
    warningLines: 1000,
    goodChars: 18_000,
    moderateChars: 36_000,
    warningChars: 60_000
};

export const CONNECTION_RELATIONS = [
    "related_to",
    "part_of",
    "depends_on",
    "extends",
    "references",
    "supports",
    "contradicts",
    "alternative_to"
] as const;

export type ConnectionRelation = (typeof CONNECTION_RELATIONS)[number];
