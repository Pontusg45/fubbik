import { useMemo } from "react";

interface DiffLine {
    type: "added" | "removed" | "unchanged";
    text: string;
}

/**
 * Simple line-based diff using a longest common subsequence algorithm.
 */
function computeDiff(oldText: string, newText: string): DiffLine[] {
    const oldLines = oldText.split("\n");
    const newLines = newText.split("\n");

    const m = oldLines.length;
    const n = newLines.length;

    // Build LCS table
    const dp: number[][] = Array.from({ length: m + 1 }, () => Array(n + 1).fill(0) as number[]);
    for (let i = 1; i <= m; i++) {
        for (let j = 1; j <= n; j++) {
            if (oldLines[i - 1] === newLines[j - 1]) {
                dp[i]![j] = dp[i - 1]![j - 1]! + 1;
            } else {
                dp[i]![j] = Math.max(dp[i - 1]![j]!, dp[i]![j - 1]!);
            }
        }
    }

    // Backtrack to produce diff
    const result: DiffLine[] = [];
    let i = m;
    let j = n;
    while (i > 0 || j > 0) {
        if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
            result.push({ type: "unchanged", text: oldLines[i - 1]! });
            i--;
            j--;
        } else if (j > 0 && (i === 0 || dp[i]![j - 1]! >= dp[i - 1]![j]!)) {
            result.push({ type: "added", text: newLines[j - 1]! });
            j--;
        } else {
            result.push({ type: "removed", text: oldLines[i - 1]! });
            i--;
        }
    }

    return result.reverse();
}

interface DiffViewerProps {
    oldText: string;
    newText: string;
    oldLabel?: string;
    newLabel?: string;
}

export function DiffViewer({ oldText, newText, oldLabel, newLabel }: DiffViewerProps) {
    const diff = useMemo(() => computeDiff(oldText, newText), [oldText, newText]);

    const addedCount = diff.filter(d => d.type === "added").length;
    const removedCount = diff.filter(d => d.type === "removed").length;

    return (
        <div className="rounded-md border">
            <div className="flex items-center justify-between border-b px-3 py-2 text-xs text-muted-foreground">
                <div className="flex gap-3">
                    {oldLabel && <span className="text-red-500">{oldLabel}</span>}
                    {newLabel && <span className="text-green-500">{newLabel}</span>}
                </div>
                <div className="flex gap-2">
                    {addedCount > 0 && <span className="text-green-500">+{addedCount}</span>}
                    {removedCount > 0 && <span className="text-red-500">-{removedCount}</span>}
                </div>
            </div>
            <div className="overflow-x-auto font-mono text-xs">
                {diff.map((line, i) => (
                    <div
                        key={i}
                        className={
                            line.type === "added"
                                ? "bg-green-500/10 text-green-700 dark:text-green-400"
                                : line.type === "removed"
                                  ? "bg-red-500/10 text-red-700 dark:text-red-400"
                                  : "text-muted-foreground"
                        }
                    >
                        <span className="inline-block w-6 select-none text-right opacity-50">
                            {line.type === "added" ? "+" : line.type === "removed" ? "-" : " "}
                        </span>
                        <span className="pl-2">{line.text || "\u00A0"}</span>
                    </div>
                ))}
                {diff.length === 0 && <div className="px-3 py-4 text-center text-muted-foreground">No differences</div>}
            </div>
        </div>
    );
}
