import { useQuery } from "@tanstack/react-query";
import { History } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Card, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function VersionHistory({ chunkId }: { chunkId: string }) {
    const [open, setOpen] = useState(false);

    const historyQuery = useQuery({
        queryKey: ["chunk-history", chunkId],
        queryFn: async () => {
            return unwrapEden(await api.api.chunks({ id: chunkId }).history.get());
        },
        enabled: open
    });

    const versions = historyQuery.data ?? [];

    return (
        <Card>
            <CardHeader>
                <button
                    type="button"
                    onClick={() => setOpen(!open)}
                    className="flex w-full items-center justify-between"
                >
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
                    {historyQuery.isLoading && (
                        <p className="text-muted-foreground text-sm">Loading history...</p>
                    )}
                    {versions.length === 0 && !historyQuery.isLoading && (
                        <p className="text-muted-foreground text-sm">No previous versions</p>
                    )}
                    {versions.map(v => (
                        <div key={v.id} className="rounded-md border px-3 py-2">
                            <div className="flex items-center justify-between">
                                <span className="text-sm font-medium">v{v.version}: {v.title}</span>
                                <span className="text-muted-foreground text-xs">
                                    {new Date(v.createdAt).toLocaleString()}
                                </span>
                            </div>
                            <p className="text-muted-foreground mt-1 line-clamp-2 text-xs">{v.content}</p>
                        </div>
                    ))}
                </CardPanel>
            )}
        </Card>
    );
}
