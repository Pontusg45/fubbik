
import { ChevronDown, ChevronRight } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { INDEX_FILE_NAMES, type FileConfig, type FileEntry, type PreviewFileResult } from "../types";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function hashContent(content: string): Promise<string> {
    const data = new TextEncoder().encode(content);
    const buffer = await crypto.subtle.digest("SHA-256", data);
    return Array.from(new Uint8Array(buffer))
        .map(b => b.toString(16).padStart(2, "0"))
        .join("");
}

interface Connection {
    source: string;
    target: string;
}

/**
 * Mirrors the backend createFolderConnections logic.
 * Groups files by directory, finds index files per directory,
 * and creates part_of connections for non-index files to their directory's index.
 */
function computeFolderConnections(paths: string[]): Connection[] {
    const byDir = new Map<string, string[]>();

    for (const p of paths) {
        const dir = p.includes("/") ? p.substring(0, p.lastIndexOf("/")) : "";
        const existing = byDir.get(dir) ?? [];
        existing.push(p);
        byDir.set(dir, existing);
    }

    const connections: Connection[] = [];

    for (const [, filePaths] of byDir) {
        const indexFile = filePaths.find(p => {
            const name = p.split("/").pop()?.toLowerCase() ?? "";
            return INDEX_FILE_NAMES.has(name);
        });

        if (!indexFile) continue;

        for (const p of filePaths) {
            if (p !== indexFile) {
                connections.push({ source: p, target: indexFile });
            }
        }
    }

    return connections;
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface StepReviewProps {
    files: FileEntry[];
    selectedPaths: Set<string>;
    preview: PreviewFileResult[];
    overrides: Map<string, FileConfig>;
    existingHashes: Record<string, string>;
    codebaseName: string;
    onGoToFile: (path: string) => void;
}

// ---------------------------------------------------------------------------
// StepReview component
// ---------------------------------------------------------------------------

export function StepReview({
    files,
    selectedPaths,
    preview,
    overrides,
    existingHashes,
    codebaseName,
    onGoToFile
}: StepReviewProps) {
    const [computedHashes, setComputedHashes] = useState<Record<string, string>>({});
    const [newExpanded, setNewExpanded] = useState(true);
    const [skippedExpanded, setSkippedExpanded] = useState(false);
    const [connectionsExpanded, setConnectionsExpanded] = useState(true);

    // Compute client-side SHA-256 hashes of selected files
    useEffect(() => {
        const selectedFiles = files.filter(f => selectedPaths.has(f.path));
        if (selectedFiles.length === 0) return;

        let cancelled = false;

        const run = async () => {
            const entries = await Promise.all(
                selectedFiles.map(async f => [f.path, await hashContent(f.content)] as const)
            );
            if (!cancelled) {
                setComputedHashes(Object.fromEntries(entries));
            }
        };

        void run();

        return () => {
            cancelled = true;
        };
    }, [files, selectedPaths]);

    // Split selected files into new vs. skipped based on hash comparison
    const { newFiles, skippedFiles } = useMemo(() => {
        const selectedPreviewed = preview.filter(pf => selectedPaths.has(pf.path));
        const newList: PreviewFileResult[] = [];
        const skippedList: PreviewFileResult[] = [];

        for (const pf of selectedPreviewed) {
            const computed = computedHashes[pf.path];
            const existing = existingHashes[pf.path];
            if (computed && existing && computed === existing) {
                skippedList.push(pf);
            } else {
                newList.push(pf);
            }
        }

        return { newFiles: newList, skippedFiles: skippedList };
    }, [preview, selectedPaths, computedHashes, existingHashes]);

    const templateMatchCount = useMemo(() => {
        let count = 0;
        for (const pf of newFiles) {
            const config = overrides.get(pf.path);
            if (config?.templateId) count++;
        }
        return count;
    }, [newFiles, overrides]);

    const connections = useMemo(() => {
        return computeFolderConnections(newFiles.map(f => f.path));
    }, [newFiles]);

    return (
        <div className="flex flex-col gap-6">
            {/* Summary stats */}
            <div className="grid grid-cols-4 gap-3">
                <StatCard
                    value={newFiles.length}
                    label="New chunks"
                    colorClass="text-emerald-600"
                    bgClass="bg-emerald-50 dark:bg-emerald-950/30 border-emerald-200 dark:border-emerald-800"
                />
                <StatCard
                    value={skippedFiles.length}
                    label="Will be skipped"
                    subtitle="already imported"
                    colorClass="text-amber-600"
                    bgClass="bg-amber-50 dark:bg-amber-950/30 border-amber-200 dark:border-amber-800"
                />
                <StatCard
                    value={templateMatchCount}
                    label="Template matches"
                    colorClass="text-indigo-600"
                    bgClass="bg-indigo-50 dark:bg-indigo-950/30 border-indigo-200 dark:border-indigo-800"
                />
                <StatCard
                    value={connections.length}
                    label="Connections"
                    subtitle="part_of"
                    colorClass="text-purple-600"
                    bgClass="bg-purple-50 dark:bg-purple-950/30 border-purple-200 dark:border-purple-800"
                />
            </div>

            {/* New chunks section */}
            <CollapsibleSection
                title={`New chunks (${newFiles.length})`}
                expanded={newExpanded}
                onToggle={() => setNewExpanded(v => !v)}
                headerColorClass="text-emerald-700 dark:text-emerald-400"
                borderColorClass="border-emerald-200 dark:border-emerald-800"
            >
                {newFiles.length === 0 ? (
                    <p className="text-sm text-muted-foreground px-3 py-2">
                        No new chunks to import.
                    </p>
                ) : (
                    <div className="overflow-x-auto">
                        <table className="w-full text-sm">
                            <thead>
                                <tr className="border-b text-left text-muted-foreground">
                                    <th className="px-3 py-2 font-medium">Path</th>
                                    <th className="px-3 py-2 font-medium">Title</th>
                                    <th className="px-3 py-2 font-medium">Type</th>
                                    <th className="px-3 py-2 font-medium">Template</th>
                                    <th className="px-3 py-2 font-medium">Tags</th>
                                </tr>
                            </thead>
                            <tbody>
                                {newFiles.map(pf => {
                                    const config = overrides.get(pf.path);
                                    const hasTemplate = Boolean(config?.templateId);
                                    return (
                                        <tr
                                            key={pf.path}
                                            className={`border-b last:border-0 cursor-pointer hover:bg-muted/50 transition-colors ${
                                                hasTemplate
                                                    ? "bg-green-50/50 dark:bg-green-950/20"
                                                    : ""
                                            }`}
                                            onClick={() => onGoToFile(pf.path)}
                                        >
                                            <td className="px-3 py-2 font-mono text-xs text-muted-foreground max-w-[180px] truncate">
                                                {pf.path}
                                            </td>
                                            <td className="px-3 py-2 max-w-[200px] truncate">
                                                {config?.title ?? pf.title}
                                            </td>
                                            <td className="px-3 py-2">
                                                <Badge variant="secondary" className="text-xs">
                                                    {config?.type ?? pf.parsed.type}
                                                </Badge>
                                            </td>
                                            <td className="px-3 py-2 text-xs text-muted-foreground">
                                                {config?.templateId
                                                    ? pf.suggestedTemplate?.name ?? "—"
                                                    : "—"}
                                            </td>
                                            <td className="px-3 py-2">
                                                <div className="flex flex-wrap gap-1">
                                                    {(config?.tags ?? pf.parsed.tags)
                                                        .slice(0, 3)
                                                        .map(tag => (
                                                            <Badge
                                                                key={tag}
                                                                variant="outline"
                                                                className="text-xs px-1 py-0"
                                                            >
                                                                {tag}
                                                            </Badge>
                                                        ))}
                                                    {(config?.tags ?? pf.parsed.tags).length >
                                                        3 && (
                                                        <span className="text-xs text-muted-foreground">
                                                            +
                                                            {(config?.tags ?? pf.parsed.tags)
                                                                .length - 3}
                                                        </span>
                                                    )}
                                                </div>
                                            </td>
                                        </tr>
                                    );
                                })}
                            </tbody>
                        </table>
                    </div>
                )}
            </CollapsibleSection>

            {/* Skipped files section */}
            <CollapsibleSection
                title={`Skipped files (${skippedFiles.length})`}
                expanded={skippedExpanded}
                onToggle={() => setSkippedExpanded(v => !v)}
                headerColorClass="text-amber-700 dark:text-amber-400"
                borderColorClass="border-amber-200 dark:border-amber-800"
            >
                {skippedFiles.length === 0 ? (
                    <p className="text-sm text-muted-foreground px-3 py-2">
                        No files will be skipped.
                    </p>
                ) : (
                    <div className="flex flex-col gap-1 px-3 py-2">
                        <p className="text-sm text-muted-foreground mb-2">
                            These files were already imported with identical content and will be
                            skipped during import.
                        </p>
                        {skippedFiles.map(pf => (
                            <div
                                key={pf.path}
                                className="flex items-center gap-2 text-sm"
                            >
                                <span className="font-mono text-xs text-muted-foreground">
                                    {pf.path}
                                </span>
                            </div>
                        ))}
                    </div>
                )}
            </CollapsibleSection>

            {/* Connections section */}
            <CollapsibleSection
                title={`Connections (${connections.length})`}
                expanded={connectionsExpanded}
                onToggle={() => setConnectionsExpanded(v => !v)}
                headerColorClass="text-purple-700 dark:text-purple-400"
                borderColorClass="border-purple-200 dark:border-purple-800"
            >
                {connections.length === 0 ? (
                    <p className="text-sm text-muted-foreground px-3 py-2">
                        No folder connections detected. Add an index.md or readme.md file to a
                        folder to create part_of connections.
                    </p>
                ) : (
                    <div className="flex flex-col gap-1 px-3 py-2">
                        {connections.map((c, i) => (
                            <div key={i} className="flex items-center gap-2 text-sm">
                                <span className="font-mono text-xs text-muted-foreground truncate max-w-[200px]">
                                    {c.source}
                                </span>
                                <span className="text-purple-500 shrink-0">→</span>
                                <span className="text-xs text-purple-600 dark:text-purple-400 shrink-0">
                                    part_of
                                </span>
                                <span className="text-purple-500 shrink-0">→</span>
                                <span className="font-mono text-xs text-muted-foreground truncate max-w-[200px]">
                                    {c.target}
                                </span>
                            </div>
                        ))}
                    </div>
                )}
            </CollapsibleSection>

            {/* Codebase reminder */}
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <span>Importing into:</span>
                <Badge variant="secondary">{codebaseName}</Badge>
            </div>
        </div>
    );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface StatCardProps {
    value: number;
    label: string;
    subtitle?: string;
    colorClass: string;
    bgClass: string;
}

function StatCard({ value, label, subtitle, colorClass, bgClass }: StatCardProps) {
    return (
        <div className={`rounded-lg border p-3 ${bgClass}`}>
            <div className={`text-2xl font-bold ${colorClass}`}>{value}</div>
            <div className="text-sm font-medium">{label}</div>
            {subtitle && (
                <div className="text-xs text-muted-foreground">{subtitle}</div>
            )}
        </div>
    );
}

interface CollapsibleSectionProps {
    title: string;
    expanded: boolean;
    onToggle: () => void;
    headerColorClass: string;
    borderColorClass: string;
    children: React.ReactNode;
}

function CollapsibleSection({
    title,
    expanded,
    onToggle,
    headerColorClass,
    borderColorClass,
    children
}: CollapsibleSectionProps) {
    return (
        <div className={`rounded-lg border ${borderColorClass} overflow-hidden`}>
            <button
                type="button"
                className={`flex w-full items-center gap-2 px-3 py-2.5 text-sm font-medium hover:bg-muted/30 transition-colors ${headerColorClass}`}
                onClick={onToggle}
            >
                {expanded ? (
                    <ChevronDown className="size-4 shrink-0" />
                ) : (
                    <ChevronRight className="size-4 shrink-0" />
                )}
                {title}
            </button>
            {expanded && <div className="border-t">{children}</div>}
        </div>
    );
}
