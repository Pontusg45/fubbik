import { useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { CheckCircle2, Loader2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { Button } from "@/components/ui/button";

import type { FileConfig, FileEntry, ImportFileStatus } from "../types";
import { useSSEImport } from "../use-sse-import";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface StepImportProps {
    files: FileEntry[];
    selectedPaths: Set<string>;
    spaceId: string;
    overrides: Map<string, FileConfig>;
    importStatus: Map<string, ImportFileStatus>;
    onStatusChange: React.Dispatch<React.SetStateAction<Map<string, ImportFileStatus>>>;
    onReset: () => void;
}

// ---------------------------------------------------------------------------
// StepImport component
// ---------------------------------------------------------------------------

export function StepImport({ files, selectedPaths, spaceId, overrides, importStatus, onStatusChange, onReset }: StepImportProps) {
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

    const selectedFiles = useMemo(() => files.filter(f => selectedPaths.has(f.path)), [files, selectedPaths]);

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
            spaceId,
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
                    elapsed: 0
                });
                console.error("Import error:", error);
            }
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
                    <div className="text-muted-foreground flex items-center justify-between text-sm">
                        <span>Importing files…</span>
                        <span>
                            {processedCount} / {totalCount}
                        </span>
                    </div>
                    <div className="bg-muted h-2 w-full overflow-hidden rounded-full">
                        <div
                            className="bg-primary h-full rounded-full transition-all duration-300 ease-out"
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
                <div className="flex flex-col gap-4 rounded-lg border border-emerald-200 bg-emerald-50 p-4 dark:border-emerald-800 dark:bg-emerald-950/30">
                    <div className="flex items-center gap-3">
                        <CheckCircle2 className="size-6 shrink-0 text-emerald-600" />
                        <div>
                            <div className="font-semibold text-emerald-700 dark:text-emerald-400">Import Complete</div>
                            {elapsedLabel && <div className="text-muted-foreground text-xs">Finished in {elapsedLabel}</div>}
                        </div>
                    </div>

                    <div className="grid grid-cols-4 gap-3">
                        <StatCard
                            value={completionData.created}
                            label="Created"
                            colorClass="text-emerald-600"
                            bgClass="bg-white dark:bg-emerald-900/20 border-emerald-200 dark:border-emerald-700"
                        />
                        <StatCard
                            value={completionData.skipped}
                            label="Skipped"
                            colorClass="text-amber-600"
                            bgClass="bg-white dark:bg-amber-900/20 border-amber-200 dark:border-amber-700"
                        />
                        <StatCard
                            value={completionData.errors}
                            label="Errors"
                            colorClass="text-red-600"
                            bgClass="bg-white dark:bg-red-900/20 border-red-200 dark:border-red-700"
                        />
                        <StatCard
                            value={completionData.connections}
                            label="Connections"
                            colorClass="text-purple-600"
                            bgClass="bg-white dark:bg-purple-900/20 border-purple-200 dark:border-purple-700"
                        />
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

            {/* Connection lost warning banner */}
            {Array.from(importStatus.values()).some(s => s.error?.includes("Connection lost")) && (
                <div className="mb-4 rounded-lg border border-amber-500/20 bg-amber-500/5 px-4 py-3 text-sm text-amber-500">
                    Connection lost — some files may have been imported. Check{" "}
                    <Link to="/chunks" className="underline">
                        chunks
                    </Link>{" "}
                    for results.
                </div>
            )}

            {/* Pipeline table */}
            <div className="overflow-hidden rounded-md border">
                <div className="overflow-x-auto">
                    <table className="w-full text-sm">
                        <thead>
                            <tr className="bg-muted/30 text-muted-foreground border-b text-left">
                                <th className="w-8 px-3 py-2 font-medium" />
                                <th className="px-3 py-2 font-medium">File path</th>
                                <th className="px-3 py-2 font-medium">Title</th>
                                <th className="w-20 px-3 py-2 font-medium">Chunks</th>
                                <th className="px-3 py-2 font-medium">Detail</th>
                            </tr>
                        </thead>
                        <tbody>
                            {selectedFiles.map(f => {
                                const s = importStatus.get(f.path) ?? { status: "pending" as const };
                                const config = overrides.get(f.path);
                                return <ImportRow key={f.path} path={f.path} title={config?.title ?? f.path} status={s} />;
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
        <tr data-path={path} className={`border-b transition-colors last:border-0 ${rowClass}`}>
            <td className="px-3 py-2 text-center">{icon}</td>
            <td className="text-muted-foreground max-w-[200px] truncate px-3 py-2 font-mono text-xs">{path}</td>
            <td className="max-w-[200px] truncate px-3 py-2 text-sm">{title}</td>
            <td className="text-muted-foreground px-3 py-2 text-sm">{status.created != null ? status.created : "—"}</td>
            <td className="max-w-[200px] truncate px-3 py-2 text-sm">{detailText}</td>
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
                icon: <span className="font-bold text-emerald-600">✓</span>,
                detailText: <span className="text-emerald-600">Created</span>,
                rowClass: "bg-emerald-50/40 dark:bg-emerald-950/20"
            };
        case "skipped":
            return {
                icon: <span className="text-amber-500">○</span>,
                detailText: <span className="text-amber-600">Unchanged</span>,
                rowClass: ""
            };
        case "error":
            return {
                icon: <span className="font-bold text-red-600">✕</span>,
                detailText: (
                    <span className="block max-w-[200px] truncate text-red-600" title={status.error}>
                        {status.error ?? "Error"}
                    </span>
                ),
                rowClass: "bg-red-50/40 dark:bg-red-950/20"
            };
        case "importing":
            return {
                icon: <Loader2 className="text-primary size-4 animate-spin" />,
                detailText: <span className="text-muted-foreground">Importing…</span>,
                rowClass: "bg-primary/5"
            };
        case "pending":
        default:
            return {
                icon: <span className="text-muted-foreground/50">⋯</span>,
                detailText: <span className="text-muted-foreground/60">Pending</span>,
                rowClass: ""
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
            <span className="text-muted-foreground text-xs">{label}</span>
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
