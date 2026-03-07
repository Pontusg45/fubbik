import { DEFAULT_THRESHOLDS, type ChunkSizeThresholds } from "@fubbik/api/chunk-size";

export type { ChunkSizeThresholds } from "@fubbik/api/chunk-size";
export { DEFAULT_THRESHOLDS } from "@fubbik/api/chunk-size";

export type ChunkSizeLevel = "good" | "moderate" | "warning" | "critical";

export interface ChunkSizeInfo {
    lines: number;
    chars: number;
    level: ChunkSizeLevel;
    label: string;
    color: string;
}

const LEVEL_CONFIG: Record<ChunkSizeLevel, { label: string; color: string }> = {
    good: { label: "Good", color: "#22c55e" },
    moderate: { label: "Moderate", color: "#f59e0b" },
    warning: { label: "Warning", color: "#f97316" },
    critical: { label: "Too large", color: "#ef4444" }
};

export function getChunkSize(content: string, thresholds: ChunkSizeThresholds = DEFAULT_THRESHOLDS): ChunkSizeInfo {
    const lines = content.split("\n").length;
    const chars = content.length;

    let level: ChunkSizeLevel;
    if (lines <= thresholds.goodLines && chars <= thresholds.goodChars) {
        level = "good";
    } else if (lines <= thresholds.moderateLines && chars <= thresholds.moderateChars) {
        level = "moderate";
    } else if (lines <= thresholds.warningLines && chars <= thresholds.warningChars) {
        level = "warning";
    } else {
        level = "critical";
    }

    const config = LEVEL_CONFIG[level];
    return { lines, chars, level, label: config.label, color: config.color };
}
