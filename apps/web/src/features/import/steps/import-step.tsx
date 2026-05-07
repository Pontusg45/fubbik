"use client";

import { CheckCircle2, Loader2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Button } from "@/components/ui/button";
import type { FileConfig, FileEntry, ImportFileStatus } from "../types";
import { useSSEImport } from "../use-sse-import";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface StepImportProps {
    files: FileEntry[];
    selectedPaths: Set<string>;
    codebaseId: string;
    overrides: Map<string, FileConfig>;
    importStatus: Map<string, ImportFileStatus>;
    onStatusChange: React.Dispatch<React.SetStateAction<Map<string, ImportFileStatus>>>;
    onReset: () => void;
}

// ---------------------------------------------------------------------------
// StepImport component
// ---------------------------------------------------------------------------

export function StepImport({
    files,
    selectedPaths,
    codebaseId,
    overrides,
    importStatus,
    onStatusChange,
    onReset,
}: StepImportProps) {
    const queryClient = useQueryClient();
    const { startImport } = useSSEImport();
    const startedRef = useRef(false);
    const [completionData, setCompletionData] = useState<{
        created: number;
        skipped: number;
        errors: number;
        connections: number;
        elapsed: number;
    } | null>(null);

    const selectedFiles = useMemo(
        () => files.filter(f => selectedPaths.has(f.path)),
        [files, selectedPaths]
    );

    // Start import on mount (once)
    useEffect(() => {
        if (startedRef.current) return;
        startedRef.current = true;

        // Initialize all selected files as pending
        const initial = new Map<string, ImportFileStatus>();
        for (const f of selectedFiles) {
            initial.set(f.path, { status: "pending" });
        }
        onStatusChange(initial);

        // Build templateOverrides from overrides Map
        const templateOverrides: Record<string, string | null> = {};
        for (const [path, config] of overrides.entries()) {
            if (selectedPaths.has(path)) {
                templateOverrides[path] = config.templateId;
            }
        }

        void startImport({
            files: selectedFiles.map(f => ({ path: f.path, content: f.content })),
            codebaseId,
            templateOverrides,
            onFileUpdate: (path, status) => {
                onStatusChange(prev => {
                    const next = new Map(prev);
                    next.set(path, status);
                    return next;
                });
            },
            onDone: result => {
                setCompletionData(result);
                void queryClient.invalidateQueries({ queryKey: ["chunks"] });
            },
            onError: error => {
                // Mark all still-pending files as errors
                onStatusChange(prev => {
                    const next = new Map(prev);
                    for (const [path, s] of next.entries()) {
                        if (s.status === "pending" || s.status === "importing") {
                            next.set(path, { status: "error", error: "Connection lost" });
                        }
                    }
                    return next;
                });
                // Also store completion data with the error
                setCompletionData({
                    created: 0,
                    skipped: 0,
                    errors: selectedFiles.length,
                    connections: 0,
                    elapsed: 0,
                });
                console.error("Import error:", error);
            },
        });
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    // Auto-scroll currently importing row into view
    useEffect(() => {
        for (const [path, s] of importStatus.entries()) {
            if (s.status === "importing") {
                const el = document.querySelector(`[data-path="${CSS.escape(path)}"]`);
                el?.scrollIntoView({ behavior: "smooth", block: "nearest" });
                break;
            }
        }
    }, [importStatus]);

    // Compute stats
    const stats = useMemo(() => {
        let created = 0;
        let skipped = 0;
        let error = 0;
        let pending = 0;
        for (const s of importStatus.values()) {
            if (s.status === "created") created++;
            else if (s.status === "skipped") skipped++;
            else if (s.status === "error") error++;
            else if (s.status === "pending" || s.status === "importing") pending++;
        }
        return { created, skipped, error, pending };
    }, [importStatus]);

    const totalCount = selectedFiles.length;
    const processedCount = stats.created + stats.skipped + stats.error;

    // Elapsed time formatted
    const elapsedLabel = completionData
        ? completionData.elapsed >= 1000
            ? `${(completionData.elapsed / 1000).toFixed(1)}s`
            : `${completionData.elapsed}ms`
        : null;

    return (
        <div className="flex flex-col gap-6">
            {/* Progress bar */}
            {!completionData && (
                <div className="flex flex-col gap-2">
                    <div className="flex items-center justify-between text-sm text-muted-foreground">
                        <span>Importing files…</span>
                        <span>{processedCount} / {totalCount}</span>
                    </div>
                    <div className="h-2 w-full rounded-full bg-muted overflow-hidden">
                        <div
                            className="h-full bg-primary rounded-full transition-all duration-300 ease-out"
                            style={{ width: totalCount > 0 ? `${(processedCount / totalCount) * 100}%` : "0%" }}
                        />
                    </div>
                </div>
            )}

            {/* Live stats */}
            {!completionData && (
                <div className="grid grid-cols-4 gap-3">
                    <StatBadge value={stats.created} label="Created" colorClass="text-emerald-600" />
                    <StatBadge value={stats.skipped} label="Unchanged" colorClass="text-amber-600" />
                    <StatBadge value={stats.error} label="Errors" colorClass="text-red-600" />
                    <StatBadge value={stats.pending} label="Pending" colorClass="text-muted-foreground" />
                </div>
            )}

            {/* Completion banner */}
            {completionData && (
                <div className="rounded-lg border border-emerald-200 dark:border-emerald-800 bg-emerald-50 dark:bg-emerald-950/30 p-4 flex flex-col gap-4">
                    <div className="flex items-center gap-3">
                        <CheckCircle2 className="size-6 text-emerald-600 shrink-0" />
                        <div>
                            <div className="font-semibold text-emerald-700 dark:text-emerald-400">
                                Import Complete
                            </div>
                            {elapsedLabel && (
                                <div className="text-xs text-muted-foreground">
                                    Finished in {elapsedLabel}
                                </div>
                            )}
                        </div>
                    </div>

                    <div className="grid grid-cols-4 gap-3">
                        <StatCard value={completionData.created} label="Created" colorClass="text-emerald-600" bgClass="bg-white dark:bg-emerald-900/20 border-emerald-200 dark:border-emerald-700" />
                        <StatCard value={completionData.skipped} label="Skipped" colorClass="text-amber-600" bgClass="bg-white dark:bg-amber-900/20 border-amber-200 dark:border-amber-700" />
                        <StatCard value={completionData.errors} label="Errors" colorClass="text-red-600" bgClass="bg-white dark:bg-red-900/20 border-red-200 dark:border-red-700" />
                        <StatCard value={completionData.connections} label="Connections" colorClass="text-purple-600" bgClass="bg-white dark:bg-purple-900/20 border-purple-200 dark:border-purple-700" />
                    </div>

                    <div className="flex items-center gap-3 pt-1">
                        <Button size="sm" render={<Link to="/chunks" />}>
                            View imported chunks →
                        </Button>
                        <Button variant="outline" size="sm" render={<Link to="/graph" />}>
                            View in graph
                        </Button>
                        <Button variant="ghost" size="sm" onClick={onReset}>
                            Import more
                        </Button>
                    </div>
                </div>
            )}

            {/* Pipeline table */}
            <div className="rounded-md border overflow-hidden">
                <div className="overflow-x-auto">
                    <table className="w-full text-sm">
                        <thead>
                            <tr className="border-b bg-muted/30 text-left text-muted-foreground">
                                <th className="px-3 py-2 font-medium w-8" />
                                <th className="px-3 py-2 font-medium">File path</th>
                                <th className="px-3 py-2 font-medium">Title</th>
                                <th className="px-3 py-2 font-medium w-20">Chunks</th>
                                <th className="px-3 py-2 font-medium">Detail</th>
                            </tr>
                        </thead>
                        <tbody>
                            {selectedFiles.map(f => {
                                const s = importStatus.get(f.path) ?? { status: "pending" as const };
                                const config = overrides.get(f.path);
                                return (
                                    <ImportRow
                                        key={f.path}
                                        path={f.path}
                                        title={config?.title ?? f.path}
                                        status={s}
                                    />
                                );
                            })}
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    );
}

// ---------------------------------------------------------------------------
// ImportRow
// ---------------------------------------------------------------------------

interface ImportRowProps {
    path: string;
    title: string;
    status: ImportFileStatus;
}

function ImportRow({ path, title, status }: ImportRowProps) {
    const { icon, detailText, rowClass } = getRowStyle(status);

    return (
        <tr
            data-path={path}
            className={`border-b last:border-0 transition-colors ${rowClass}`}
        >
            <td className="px-3 py-2 text-center">{icon}</td>
            <td className="px-3 py-2 font-mono text-xs text-muted-foreground max-w-[200px] truncate">
                {path}
            </td>
            <td className="px-3 py-2 max-w-[200px] truncate text-sm">
                {title}
            </td>
            <td className="px-3 py-2 text-sm text-muted-foreground">
                {status.created != null ? status.created : "—"}
            </td>
            <td className="px-3 py-2 text-sm max-w-[200px] truncate">{detailText}</td>
        </tr>
    );
}

function getRowStyle(status: ImportFileStatus): {
    icon: React.ReactNode;
    detailText: React.ReactNode;
    rowClass: string;
} {
    switch (status.status) {
        case "created":
            return {
                icon: <span className="text-emerald-600 font-bold">✓</span>,
                detailText: <span className="text-emerald-600">Created</span>,
                rowClass: "bg-emerald-50/40 dark:bg-emerald-950/20",
            };
        case "skipped":
            return {
                icon: <span className="text-amber-500">○</span>,
                detailText: <span className="text-amber-600">Unchanged</span>,
                rowClass: "",
            };
        case "error":
            return {
                icon: <span className="text-red-600 font-bold">✕</span>,
                detailText: (
                    <span
                        className="text-red-600 truncate block max-w-[200px]"
                        title={status.error}
                    >
                        {status.error ?? "Error"}
                    </span>
                ),
                rowClass: "bg-red-50/40 dark:bg-red-950/20",
            };
        case "importing":
            return {
                icon: <Loader2 className="size-4 animate-spin text-primary" />,
                detailText: <span className="text-muted-foreground">Importing…</span>,
                rowClass: "bg-primary/5",
            };
        case "pending":
        default:
            return {
                icon: <span className="text-muted-foreground/50">⋯</span>,
                detailText: <span className="text-muted-foreground/60">Pending</span>,
                rowClass: "",
            };
    }
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface StatBadgeProps {
    value: number;
    label: string;
    colorClass: string;
}

function StatBadge({ value, label, colorClass }: StatBadgeProps) {
    return (
        <div className="flex flex-col items-center rounded-md border px-3 py-2">
            <span className={`text-xl font-bold tabular-nums ${colorClass}`}>{value}</span>
            <span className="text-xs text-muted-foreground">{label}</span>
        </div>
    );
}

interface StatCardProps {
    value: number;
    label: string;
    colorClass: string;
    bgClass: string;
}

function StatCard({ value, label, colorClass, bgClass }: StatCardProps) {
    return (
        <div className={`rounded-lg border p-3 ${bgClass}`}>
            <div className={`text-2xl font-bold tabular-nums ${colorClass}`}>{value}</div>
            <div className="text-sm font-medium">{label}</div>
        </div>
    );
}
