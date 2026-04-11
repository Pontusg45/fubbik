import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ArrowLeft, BookOpen } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/learn/$pathId")({
    component: LearnPathDetail,
});

function LearnPathDetail() {
    const { pathId } = Route.useParams();

    const { data: path } = useQuery({
        queryKey: ["learning-path", pathId],
        queryFn: async () => unwrapEden(await (api.api as any)["learning-paths"]({ id: pathId }).get()),
    });

    const { data: allChunks } = useQuery({
        queryKey: ["chunks-for-path"],
        queryFn: async () =>
            unwrapEden(await api.api.chunks.get({ query: { limit: "1000" } as any })),
        staleTime: 300_000,
    });

    if (!path) {
        return (
            <div className="container mx-auto max-w-4xl px-4 py-8">
                <p className="text-muted-foreground">Loading…</p>
            </div>
        );
    }

    const chunksMap = new Map(
        ((allChunks as any)?.chunks ?? []).map((c: any) => [c.id, c]),
    );
    const pathChunks = (path as any).chunkIds
        .map((id: string) => chunksMap.get(id))
        .filter(Boolean) as Array<{ id: string; title: string; type: string }>;

    return (
        <div className="container mx-auto max-w-4xl px-4 py-8">
            <Button
                variant="ghost"
                size="sm"
                render={<Link to="/learn" />}
                className="mb-4 gap-1.5"
            >
                <ArrowLeft className="size-3.5" />
                All paths
            </Button>

            <div className="mb-8 flex items-start gap-3">
                <BookOpen className="mt-1 size-6 text-muted-foreground shrink-0" />
                <div>
                    <h1 className="text-2xl font-bold tracking-tight">{(path as any).title}</h1>
                    {(path as any).description && (
                        <p className="mt-2 text-sm text-muted-foreground">{(path as any).description}</p>
                    )}
                    <p className="mt-2 text-xs text-muted-foreground">
                        {pathChunks.length} chunks in sequence
                    </p>
                </div>
            </div>

            <ol className="space-y-2">
                {pathChunks.map((chunk, i) => (
                    <li key={chunk.id}>
                        <Link
                            to="/chunks/$chunkId"
                            params={{ chunkId: chunk.id }}
                            className="flex items-center gap-3 rounded-lg border bg-card p-3 hover:bg-muted/50 transition-colors"
                        >
                            <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-mono">
                                {i + 1}
                            </span>
                            <span className="flex-1 truncate text-sm">{chunk.title}</span>
                            <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                                {chunk.type}
                            </Badge>
                        </Link>
                    </li>
                ))}
            </ol>
        </div>
    );
}
