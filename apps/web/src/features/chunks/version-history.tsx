import { useQuery } from "@tanstack/react-query";
import { History } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { DiffViewer } from "@/features/chunks/diff-viewer";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface VersionEntry {
    id: string;
    version: number;
    title: string;
    content: string;
    createdAt: string | Date;
}

export function VersionHistory({ chunkId }: { chunkId: string }) {
    const [open, setOpen] = useState(false);
    const [compareFrom, setCompareFrom] = useState<string>("");
    const [compareTo, setCompareTo] = useState<string>("");
    const [showDiff, setShowDiff] = useState(false);

    const historyQuery = useQuery({
        queryKey: ["chunk-history", chunkId],
        queryFn: async () => {
            return unwrapEden(await api.api.chunks({ id: chunkId }).history.get());
        },
        enabled: open
    });

    const versions = (historyQuery.data ?? []) as VersionEntry[];

    const fromVersion = versions.find(v => v.id === compareFrom);
    const toVersion = versions.find(v => v.id === compareTo);

    function handleCompare() {
        if (fromVersion && toVersion) {
            setShowDiff(true);
        }
    }

    return (
        <Card>
            <CardHeader>
                <button type="button" onClick={() => setOpen(!open)} className="flex w-full items-center justify-between">
                    <CardTitle className="flex items-center gap-2 text-sm">
                        <History className="size-4" />
                        Version History
                    </CardTitle>
                    <Badge variant="secondary" size="sm">
                        {open ? "Hide" : "Show"}
                    </Badge>
                </button>
            </CardHeader>
            {open && (
                <CardPanel className="space-y-2 pt-0">
                    {historyQuery.isLoading && <p className="text-muted-foreground text-sm">Loading history...</p>}
                    {versions.length === 0 && !historyQuery.isLoading && (
                        <p className="text-muted-foreground text-sm">No previous versions</p>
                    )}
                    {versions.map(v => (
                        <div key={v.id} className="rounded-md border px-3 py-2">
                            <div className="flex items-center justify-between">
                                <span className="text-sm font-medium">
                                    v{v.version}: {v.title}
                                </span>
                                <span className="text-muted-foreground text-xs">{new Date(v.createdAt).toLocaleString()}</span>
                            </div>
                            <p className="text-muted-foreground mt-1 line-clamp-2 text-xs">{v.content}</p>
                        </div>
                    ))}

                    {versions.length >= 2 && (
                        <>
                            <div className="border-t pt-3">
                                <p className="mb-2 text-sm font-medium">Compare Versions</p>
                                <div className="flex flex-wrap items-end gap-2">
                                    <div>
                                        <label className="mb-1 block text-xs text-muted-foreground">From (older)</label>
                                        <select
                                            value={compareFrom}
                                            onChange={e => {
                                                setCompareFrom(e.target.value);
                                                setShowDiff(false);
                                            }}
                                            className="bg-background rounded-md border px-2 py-1.5 text-xs focus:outline-none focus:ring-2 focus:ring-ring"
                                        >
                                            <option value="">Select version...</option>
                                            {versions.map(v => (
                                                <option key={v.id} value={v.id}>
                                                    v{v.version}: {v.title}
                                                </option>
                                            ))}
                                        </select>
                                    </div>
                                    <div>
                                        <label className="mb-1 block text-xs text-muted-foreground">To (newer)</label>
                                        <select
                                            value={compareTo}
                                            onChange={e => {
                                                setCompareTo(e.target.value);
                                                setShowDiff(false);
                                            }}
                                            className="bg-background rounded-md border px-2 py-1.5 text-xs focus:outline-none focus:ring-2 focus:ring-ring"
                                        >
                                            <option value="">Select version...</option>
                                            {versions.map(v => (
                                                <option key={v.id} value={v.id}>
                                                    v{v.version}: {v.title}
                                                </option>
                                            ))}
                                        </select>
                                    </div>
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        onClick={handleCompare}
                                        disabled={!compareFrom || !compareTo || compareFrom === compareTo}
                                    >
                                        Compare
                                    </Button>
                                </div>
                            </div>

                            {showDiff && fromVersion && toVersion && (
                                <DiffViewer
                                    oldText={fromVersion.content}
                                    newText={toVersion.content}
                                    oldLabel={`v${fromVersion.version}`}
                                    newLabel={`v${toVersion.version}`}
                                />
                            )}
                        </>
                    )}
                </CardPanel>
            )}
        </Card>
    );
}
