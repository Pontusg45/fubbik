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
