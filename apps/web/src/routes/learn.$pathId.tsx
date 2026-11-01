import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, BookOpen } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/learn/$pathId")({
    component: LearnPathDetail
});

function LearnPathDetail() {
    const { pathId } = Route.useParams();
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const [confirmDelete, setConfirmDelete] = useState(false);
    const deletePath = useMutation({
        mutationFn: async () => unwrapEden(await api.api["learning-paths"]({ id: pathId }).delete()),
        onSuccess: async () => {
            setConfirmDelete(false);
            await queryClient.invalidateQueries({ queryKey: ["learning-paths"] });
            toast.success("Learning path deleted");
            void navigate({ to: "/learn" });
        },
        onError: () => toast.error("Could not delete learning path")
    });

    const {
        data: path,
        isLoading: pathLoading,
        isError: pathError
    } = useQuery({
        queryKey: ["learning-path", pathId],
        queryFn: async () => unwrapEden(await api.api["learning-paths"]({ id: pathId }).get())
    });

    const { data: allChunks } = useQuery({
        queryKey: ["chunks-for-path"],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { limit: "1000" } as any })),
        staleTime: 300_000
    });

    if (pathLoading) {
        return (
            <div className="container mx-auto max-w-4xl px-4 py-8">
                <p className="text-muted-foreground">Loading…</p>
            </div>
        );
    }
    if (pathError || !path)
        return (
            <p role="alert" className="container mx-auto px-4 py-8">
                Learning path not found.
            </p>
        );

    const chunksMap = new Map(((allChunks as any)?.chunks ?? []).map((c: any) => [c.id, c]));
    const pathChunks = (path as any).chunkIds.map((id: string) => chunksMap.get(id)).filter(Boolean) as Array<{
        id: string;
        title: string;
        type: string;
    }>;

    return (
        <div className="container mx-auto max-w-4xl px-4 py-8">
            <Button variant="ghost" size="sm" render={<Link to="/learn" />} className="mb-4 gap-1.5">
                <ArrowLeft className="size-3.5" />
                All paths
            </Button>

            <div className="mb-8 flex items-start gap-3">
                <BookOpen className="text-muted-foreground mt-1 size-6 shrink-0" />
                <div>
                    <h1 className="text-2xl font-bold tracking-tight">{(path as any).title}</h1>
                    {(path as any).description && <p className="text-muted-foreground mt-2 text-sm">{(path as any).description}</p>}
                    <p className="text-muted-foreground mt-2 text-xs">{pathChunks.length} chunks in sequence</p>
                </div>
            </div>
            <div className="mb-6 flex gap-2">
                <Button variant="outline" render={<Link to="/learn/$pathId/edit" params={{ pathId }} />}>
                    Edit path
                </Button>
                <Button variant="destructive" disabled={deletePath.isPending} onClick={() => setConfirmDelete(true)}>
                    Delete path
                </Button>
            </div>
            <ConfirmDialog
                open={confirmDelete}
                onOpenChange={setConfirmDelete}
                title="Delete learning path?"
                description="This removes the path, but keeps its chunks."
                confirmLabel="Delete path"
                confirmVariant="destructive"
                loading={deletePath.isPending}
                onConfirm={() => deletePath.mutate()}
            />

            <ol className="space-y-2">
                {pathChunks.map((chunk, i) => (
                    <li key={chunk.id}>
                        <Link
                            to="/chunks/$chunkId"
                            params={{ chunkId: chunk.id }}
                            className="bg-card hover:bg-muted/50 flex items-center gap-3 rounded-lg border p-3 transition-colors"
                        >
                            <span className="bg-muted flex size-6 shrink-0 items-center justify-center rounded-full font-mono text-xs">
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
