import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Activity, FileText, Lightbulb, RefreshCw } from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { PageContainer, PageHeader, PageLoading } from "@/components/ui/page";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/knowledge-health")({
    component: KnowledgeHealthPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest access
        }
        return { session };
    }
});

function daysAgo(date: string | Date): string {
    const diff = Math.floor((Date.now() - new Date(date).getTime()) / (1000 * 60 * 60 * 24));
    if (diff === 0) return "today";
    if (diff === 1) return "1 day ago";
    return `${diff} days ago`;
}

function KnowledgeHealthPage() {
    const { codebaseId } = useActiveCodebase();

    const gapsQuery = useQuery({
        queryKey: ["knowledge-gaps"],
        queryFn: async () => {
            const result = unwrapEden(await (api.api.sessions as any)["knowledge-gaps"].get());
            // db.execute() returns { rows: [...] } — extract the rows array
            const rows = Array.isArray(result) ? result : (result as any)?.rows ?? [];
            return rows as Array<{ description: string; frequency: number; session_ids: string[] }>;
        }
    });

    const healthQuery = useQuery({
        queryKey: ["knowledge-health", codebaseId],
        queryFn: async () => {
            const query: { codebaseId?: string } = {};
            if (codebaseId) {
                query.codebaseId = codebaseId;
            }
            return unwrapEden(await api.api.health.knowledge.get({ query }));
        }
    });

    const data = healthQuery.data;

    return (
        <PageContainer maxWidth="5xl">
            <PageHeader icon={Activity} title="Knowledge Health" />

            {healthQuery.isLoading ? (
                <PageLoading count={4} />
            ) : healthQuery.isError ? (
                <p className="text-muted-foreground text-sm">Failed to load health data.</p>
            ) : data ? (
                <div className="space-y-6">
                    {/* Orphan Chunks */}
                    <Card>
                        <CardPanel className="p-6">
                            <div className="mb-3 flex items-center gap-2">
                                <h2 className="text-lg font-semibold">Orphan Chunks</h2>
                                <Badge variant="secondary">{data.orphans.count}</Badge>
                            </div>
                            <p className="text-muted-foreground mb-4 text-sm">
                                These chunks have no connections. Link them to other chunks or delete them if no longer
                                needed.
                            </p>
                            {data.orphans.chunks.length === 0 ? (
                                <p className="text-muted-foreground text-sm">No orphan chunks found.</p>
                            ) : (
                                <div className="divide-y">
                                    {data.orphans.chunks.map(c => (
                                        <div
                                            key={c.id}
                                            className="flex items-center justify-between py-3 first:pt-0 last:pb-0"
                                        >
                                            <div className="min-w-0 flex-1">
                                                <Link
                                                    to="/chunks/$chunkId"
                                                    params={{ chunkId: c.id }}
                                                    className="hover:underline font-medium"
                                                >
                                                    {c.title}
                                                </Link>
                                                <div className="mt-1 flex items-center gap-2">
                                                    <Badge variant="outline" className="text-xs">
                                                        {c.type}
                                                    </Badge>
                                                    <span className="text-muted-foreground text-xs">
                                                        Created {daysAgo(c.createdAt)}
                                                    </span>
                                                </div>
                                            </div>
                                            <Button variant="ghost" size="sm" render={<Link to="/chunks/$chunkId" params={{ chunkId: c.id }} />}>
                                                View
                                            </Button>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </CardPanel>
                    </Card>

                    {/* Stale Chunks */}
                    <Card>
                        <CardPanel className="p-6">
                            <div className="mb-3 flex items-center gap-2">
                                <h2 className="text-lg font-semibold">Stale Chunks</h2>
                                <Badge variant="secondary">{data.stale.count}</Badge>
                            </div>
                            <p className="text-muted-foreground mb-4 text-sm">
                                These chunks haven't been updated recently but are connected to chunks that have. They
                                may need a refresh.
                            </p>
                            {data.stale.chunks.length === 0 ? (
                                <p className="text-muted-foreground text-sm">No stale chunks found.</p>
                            ) : (
                                <div className="divide-y">
                                    {data.stale.chunks.map(c => (
                                        <div
                                            key={c.id}
                                            className="flex items-center justify-between py-3 first:pt-0 last:pb-0"
                                        >
                                            <div className="min-w-0 flex-1">
                                                <Link
                                                    to="/chunks/$chunkId"
                                                    params={{ chunkId: c.id }}
                                                    className="hover:underline font-medium"
                                                >
                                                    {c.title}
                                                </Link>
                                                <div className="mt-1 flex items-center gap-2">
                                                    <Badge variant="outline" className="text-xs">
                                                        {c.type}
                                                    </Badge>
                                                    <span className="text-muted-foreground text-xs">
                                                        Updated {daysAgo(c.updatedAt)}
                                                    </span>
                                                    {c.newestNeighborUpdate && (
                                                        <span className="text-muted-foreground text-xs">
                                                            Neighbor updated {daysAgo(c.newestNeighborUpdate)}
                                                        </span>
                                                    )}
                                                </div>
                                            </div>
                                            <Button variant="ghost" size="sm" render={<Link to="/chunks/$chunkId/edit" params={{ chunkId: c.id }} />}>
                                                Edit
                                            </Button>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </CardPanel>
                    </Card>

                    {/* Thin Chunks */}
                    <Card>
                        <CardPanel className="p-6">
                            <div className="mb-3 flex items-center gap-2">
                                <h2 className="text-lg font-semibold">Thin Chunks</h2>
                                <Badge variant="secondary">{data.thin.count}</Badge>
                            </div>
                            <p className="text-muted-foreground mb-4 text-sm">
                                These chunks have very little content. Consider expanding them or merging with related
                                chunks.
                            </p>
                            {data.thin.chunks.length === 0 ? (
                                <p className="text-muted-foreground text-sm">No thin chunks found.</p>
                            ) : (
                                <div className="divide-y">
                                    {data.thin.chunks.map(c => (
                                        <div
                                            key={c.id}
                                            className="flex items-center justify-between py-3 first:pt-0 last:pb-0"
                                        >
                                            <div className="min-w-0 flex-1">
                                                <Link
                                                    to="/chunks/$chunkId"
                                                    params={{ chunkId: c.id }}
                                                    className="hover:underline font-medium"
                                                >
                                                    {c.title}
                                                </Link>
                                                <div className="mt-1 flex items-center gap-2">
                                                    <Badge variant="outline" className="text-xs">
                                                        {c.type}
                                                    </Badge>
                                                    <span className="text-muted-foreground text-xs">
                                                        {c.contentLength} characters
                                                    </span>
                                                </div>
                                            </div>
                                            <Button variant="ghost" size="sm" render={<Link to="/chunks/$chunkId/edit" params={{ chunkId: c.id }} />}>
                                                Edit
                                            </Button>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </CardPanel>
                    </Card>
                    {/* Stale Embeddings */}
                    {data.staleEmbeddings && (
                        <StaleEmbeddingsCard staleEmbeddings={data.staleEmbeddings} />
                    )}

                    {/* File References */}
                    {data.fileRefs && (
                        <Card>
                            <CardPanel className="p-6">
                                <div className="mb-3 flex items-center gap-2">
                                    <FileText className="size-4 text-blue-500" />
                                    <h2 className="text-lg font-semibold">File References</h2>
                                    <Badge variant="secondary">{data.fileRefs.count}</Badge>
                                </div>
                                <p className="text-muted-foreground mb-4 text-sm">
                                    File paths referenced by chunks. Verify these paths still exist and are correct.
                                </p>
                                {data.fileRefs.refs.length === 0 ? (
                                    <p className="text-muted-foreground text-sm">No file references found.</p>
                                ) : (
                                    <div className="divide-y">
                                        {data.fileRefs.refs.map(ref => (
                                            <div
                                                key={ref.refId}
                                                className="flex items-center justify-between py-3 first:pt-0 last:pb-0"
                                            >
                                                <div className="min-w-0 flex-1">
                                                    <code className="text-sm">{ref.path}</code>
                                                    <div className="mt-1 flex items-center gap-2">
                                                        <Badge variant="outline" className="text-xs">
                                                            {ref.relation}
                                                        </Badge>
                                                        <Link
                                                            to="/chunks/$chunkId"
                                                            params={{ chunkId: ref.chunkId }}
                                                            className="text-muted-foreground hover:underline text-xs"
                                                        >
                                                            {ref.chunkTitle}
                                                        </Link>
                                                    </div>
                                                </div>
                                                <Button
                                                    variant="ghost"
                                                    size="sm"
                                                    render={
                                                        <Link
                                                            to="/chunks/$chunkId"
                                                            params={{ chunkId: ref.chunkId }}
                                                        />
                                                    }
                                                >
                                                    View chunk
                                                </Button>
                                            </div>
                                        ))}
                                    </div>
                                )}
                            </CardPanel>
                        </Card>
                    )}

                    {/* Knowledge Gaps from AI Sessions */}
                    {gapsQuery.data && gapsQuery.data.length > 0 && (
                        <Card>
                            <CardPanel className="p-6">
                                <div className="mb-3 flex items-center gap-2">
                                    <Lightbulb className="size-4 text-amber-500" />
                                    <h2 className="text-lg font-semibold">Knowledge Gaps from AI</h2>
                                    <Badge variant="secondary">{gapsQuery.data.length}</Badge>
                                </div>
                                <p className="text-muted-foreground mb-4 text-sm">
                                    These knowledge gaps were identified during AI review sessions. Consider creating chunks to address them.
                                </p>
                                <div className="divide-y">
                                    {gapsQuery.data.map((gap: any, i: number) => (
                                        <div key={i} className="flex items-center justify-between py-3 first:pt-0 last:pb-0">
                                            <div className="min-w-0 flex-1">
                                                <span className="text-sm">{gap.description}</span>
                                                <span className="text-muted-foreground ml-2 text-xs">
                                                    ({gap.frequency}x across {gap.session_ids?.length ?? 0} sessions)
                                                </span>
                                            </div>
                                            <Button variant="outline" size="sm" render={<Link to="/chunks/new" />}>
                                                Create chunk
                                            </Button>
                                        </div>
                                    ))}
                                </div>
                            </CardPanel>
                        </Card>
                    )}
                </div>
            ) : null}
        </PageContainer>
    );
}

function StaleEmbeddingsCard({
    staleEmbeddings
}: {
    staleEmbeddings: { chunks: Array<{ id: string; title: string; type: string; updatedAt: string | Date; embeddingUpdatedAt: string | Date | null }>; count: number };
}) {
    const queryClient = useQueryClient();
    const enrichMutation = useMutation({
        mutationFn: async (chunkId: string) => {
            const res = await api.api.chunks({ id: chunkId }).enrich.post();
            if (res.error) throw new Error("Enrich failed");
            return res.data;
        },
        onSuccess: () => {
            toast.success("Re-enriched chunk successfully");
            queryClient.invalidateQueries({ queryKey: ["knowledge-health"] });
        },
        onError: () => {
            toast.error("Failed to re-enrich chunk");
        }
    });

    return (
        <Card>
            <CardPanel className="p-6">
                <div className="mb-3 flex items-center gap-2">
                    <RefreshCw className="size-4 text-orange-500" />
                    <h2 className="text-lg font-semibold">Stale Embeddings</h2>
                    <Badge variant="secondary">{staleEmbeddings.count}</Badge>
                </div>
                <p className="text-muted-foreground mb-4 text-sm">
                    These chunks have been updated since their embeddings were last generated. Re-enrich them to keep
                    semantic search accurate.
                </p>
                {staleEmbeddings.chunks.length === 0 ? (
                    <p className="text-muted-foreground text-sm">No stale embeddings found.</p>
                ) : (
                    <div className="divide-y">
                        {staleEmbeddings.chunks.map(c => (
                            <div
                                key={c.id}
                                className="flex items-center justify-between py-3 first:pt-0 last:pb-0"
                            >
                                <div className="min-w-0 flex-1">
                                    <Link
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: c.id }}
                                        className="hover:underline font-medium"
                                    >
                                        {c.title}
                                    </Link>
                                    <div className="mt-1 flex items-center gap-2">
                                        <Badge variant="outline" className="text-xs">
                                            {c.type}
                                        </Badge>
                                        {c.embeddingUpdatedAt && (
                                            <span className="text-muted-foreground text-xs">
                                                Embedding {daysAgo(c.embeddingUpdatedAt)}
                                            </span>
                                        )}
                                        <span className="text-muted-foreground text-xs">
                                            Content updated {daysAgo(c.updatedAt)}
                                        </span>
                                    </div>
                                </div>
                                <Button
                                    variant="outline"
                                    size="sm"
                                    disabled={enrichMutation.isPending}
                                    onClick={() => enrichMutation.mutate(c.id)}
                                >
                                    Re-enrich
                                </Button>
                            </div>
                        ))}
                    </div>
                )}
            </CardPanel>
        </Card>
    );
}
