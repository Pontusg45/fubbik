import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { EyeOff } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function MissedChunksWidget() {
    const { data } = useQuery({
        queryKey: ["missed-chunks"],
        queryFn: async () =>
            unwrapEden(
                await api.api.chunks.get({
                    query: { sort: "oldest", limit: "5" } as any,
                }),
            ),
    });

    const chunks = ((data as any)?.chunks ?? []) as Array<{ id: string; title: string; type: string }>;

    if (chunks.length === 0) return null;

    return (
        <div className="rounded-lg border">
            <div className="border-b px-4 py-3">
                <div className="flex items-center gap-1.5">
                    <EyeOff className="size-3.5 text-muted-foreground" />
                    <h2 className="text-sm font-semibold">You might have missed this</h2>
                </div>
            </div>
            <div className="divide-y">
                {chunks.map(chunk => (
                    <Link
                        key={chunk.id}
                        to="/chunks/$chunkId"
                        params={{ chunkId: chunk.id }}
                        className="flex items-center justify-between px-4 py-3 hover:bg-muted/50 transition-colors"
                    >
                        <span className="truncate text-sm">{chunk.title}</span>
                        <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                            {chunk.type}
                        </Badge>
                    </Link>
                ))}
            </div>
        </div>
    );
}
