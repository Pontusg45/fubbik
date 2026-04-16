import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { ChunkDetailContent } from "@/features/chunks/detail/chunk-detail-content";
import { ChunkDetailTopBar } from "@/features/chunks/detail/chunk-detail-top-bar";
import { ChunkNeighbors } from "@/features/chunks/detail/chunk-neighbors";
import { ChunkMetadataPanel } from "@/features/chunks/detail/chunk-metadata-panel";
import { ChunkSiblingNavigator } from "@/features/chunks/detail/chunk-sibling-navigator";
import { MoreContextDrawer, type DrawerTab } from "@/features/chunks/detail/more-context-drawer";
import { ChunkProposalsSection } from "@/features/proposals/chunk-proposals-section";
import { useFavorites } from "@/features/chunks/use-favorites";
import { useRecentChunks } from "@/features/chunks/use-recent-chunks";
import { useRecentlyViewed } from "@/hooks/use-recently-viewed";
import { useReadingTrail } from "@/hooks/use-reading-trail";
import { useReaderSettings, getReaderClasses } from "@/hooks/use-reader-settings";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { archiveChunk } from "@/utils/api-helpers";

export const Route = createFileRoute("/chunks/$chunkId")({
    component: ChunkDetail,
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

function ChunkDetail() {
    const { chunkId } = Route.useParams();
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const { trackView } = useRecentChunks();
    const { addItem: addRecentlyViewed } = useRecentlyViewed();
    const { addVisit } = useReadingTrail();
    const { toggleFavorite, isFavorite } = useFavorites();
    const { settings: readerSettings } = useReaderSettings();
    const readerClasses = getReaderClasses(readerSettings);
    const [scrollProgress, setScrollProgress] = useState(0);
    const [drawerOpen, setDrawerOpen] = useState(false);
    const [drawerTab, setDrawerTab] = useState<DrawerTab>("links");

    useEffect(() => {
        function handleScroll() {
            const scrollTop = window.scrollY;
            const docHeight = document.documentElement.scrollHeight - window.innerHeight;
            const progress = docHeight > 0 ? (scrollTop / docHeight) * 100 : 0;
            setScrollProgress(Math.min(100, Math.max(0, progress)));
        }
        window.addEventListener("scroll", handleScroll, { passive: true });
        handleScroll();
        return () => window.removeEventListener("scroll", handleScroll);
    }, []);

    useEffect(() => {
        function handleKey(e: KeyboardEvent) {
            const tag = document.activeElement?.tagName;
            if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
            if (e.metaKey || e.ctrlKey || e.altKey) return;
            if (e.key === "m") {
                e.preventDefault();
                setDrawerOpen(prev => !prev);
            }
        }
        window.addEventListener("keydown", handleKey);
        return () => window.removeEventListener("keydown", handleKey);
    }, []);

    useEffect(() => {
        trackView(chunkId);
    }, [chunkId, trackView]);

    const { data, isLoading, error } = useQuery({
        queryKey: ["chunk", chunkId],
        queryFn: async () => {
            const { data, error } = await api.api.chunks({ id: chunkId }).get();
            if (error) throw new Error("Failed to load chunk");
            return data;
        }
    });

    // Track in recently viewed (with title/type) once data loads
    useEffect(() => {
        if (data?.chunk) {
            addRecentlyViewed({ id: data.chunk.id, title: data.chunk.title, type: data.chunk.type });
            addVisit({ id: data.chunk.id, title: data.chunk.title, type: data.chunk.type });
        }
    }, [data?.chunk?.id, data?.chunk?.title, data?.chunk?.type, addRecentlyViewed, addVisit]);

    const reviewMutation = useMutation({
        mutationFn: async (reviewStatus: "reviewed" | "approved") => {
            const { error } = await api.api.chunks({ id: chunkId }).patch({ reviewStatus });
            if (error) throw new Error("Failed to update review status");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            toast.success("Review status updated");
        },
        onError: () => {
            toast.error("Failed to update review status");
        }
    });

    const archiveMutation = useMutation({
        mutationFn: () => archiveChunk(chunkId),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            toast.success("Chunk archived");
            navigate({ to: "/chunks", search: {} });
        },
        onError: () => {
            toast.error("Failed to archive chunk");
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async () => {
            const { error } = await api.api.chunks({ id: chunkId }).delete();
            if (error) throw new Error("Failed to delete chunk");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            toast.success("Chunk deleted");
            navigate({ to: "/dashboard" });
        },
        onError: () => {
            toast.error("Failed to delete chunk");
        }
    });

    const tagMutation = useMutation({
        mutationFn: async (tags: string[]) => {
            const { error } = await api.api.chunks({ id: chunkId }).patch({ tags });
            if (error) throw new Error("Failed to update tags");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            toast.success("Tags updated");
        },
        onError: () => {
            toast.error("Failed to update tags");
        }
    });

    const toggleEntryPointMutation = useMutation({
        mutationFn: async () => {
            const isEntryPoint = !((data?.chunk as any)?.isEntryPoint);
            const { error } = await api.api.chunks({ id: chunkId }).patch({ isEntryPoint } as any);
            if (error) throw new Error("Failed to update entry point");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            queryClient.invalidateQueries({ queryKey: ["entry-points"] });
            toast.success("Entry point updated");
        },
        onError: () => {
            toast.error("Failed to update entry point");
        }
    });

    if (isLoading) {
        return (
            <div className="container mx-auto max-w-3xl px-4 py-8">
                <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="mt-8 space-y-6">
                    {/* Title skeleton */}
                    <div className="space-y-3">
                        <Skeleton className="h-5 w-20" />
                        <Skeleton className="h-8 w-2/3" />
                        <div className="flex gap-4">
                            <Skeleton className="h-3 w-32" />
                            <Skeleton className="h-3 w-32" />
                        </div>
                    </div>
                    <Separator />
                    {/* Content skeleton */}
                    <div className="space-y-3">
                        <Skeleton className="h-4 w-full" />
                        <Skeleton className="h-4 w-full" />
                        <Skeleton className="h-4 w-5/6" />
                        <Skeleton className="h-4 w-full" />
                        <Skeleton className="h-4 w-3/4" />
                        <Skeleton className="h-4 w-full" />
                        <Skeleton className="h-4 w-2/3" />
                    </div>
                    <Separator />
                    {/* Metadata skeleton */}
                    <div className="space-y-2">
                        <Skeleton className="h-5 w-32" />
                        <Skeleton className="h-12 w-full" />
                    </div>
                </div>
            </div>
        );
    }

    if (error || !data?.chunk) {
        return (
            <div className="container mx-auto max-w-3xl px-4 py-8">
                <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
                    <ArrowLeft className="size-4" />
                    Back
                </Button>
                <div className="mt-8 text-center">
                    <p className="text-muted-foreground">Chunk "{chunkId}" not found.</p>
                </div>
            </div>
        );
    }

    const chunk = data.chunk;
    const origin = (chunk as Record<string, unknown>).origin as string | undefined;
    const reviewStatus = (chunk as Record<string, unknown>).reviewStatus as string | undefined;
    const isAi = origin === "ai";
    const connections = data.connections ?? [];
    const outgoing = connections.filter(c => c.sourceId === chunkId);
    const incoming = connections.filter(c => c.sourceId !== chunkId);
    const currentCodebases = (data as Record<string, unknown>).codebases as
        | Array<{ id: string; name: string }>
        | undefined;
    const appliesTo = (data as Record<string, unknown>).appliesTo as
        | Array<{ id: string; pattern: string; note?: string | null }>
        | undefined;
    const fileReferences = (data as Record<string, unknown>).fileReferences as
        | Array<{ id: string; path: string; anchor?: string | null; relation: string }>
        | undefined;
    const tags = ((data as Record<string, unknown>).tags as Array<{ id: string; name: string }> | undefined) ?? [];
    const rationale = (chunk as Record<string, unknown>).rationale as string | null | undefined;
    const alternatives = (chunk as Record<string, unknown>).alternatives as string[] | null | undefined;
    const consequences = (chunk as Record<string, unknown>).consequences as string | null | undefined;
    const healthScore = (data as Record<string, unknown>).healthScore as
        | { total: number; breakdown: { freshness: number; completeness: number; richness: number; connectivity: number }; issues: string[] }
        | undefined;
    const isEntryPoint = (chunk as Record<string, unknown>).isEntryPoint as boolean | undefined;

    const totalSignals = connections.length + (appliesTo?.length ?? 0) + (fileReferences?.length ?? 0);

    return (
        <>
            <div className="fixed top-0 left-0 right-0 z-50 h-0.5 bg-transparent print:hidden">
                <div
                    className="h-full bg-primary transition-[width] duration-100 ease-out"
                    style={{ width: `${scrollProgress}%` }}
                />
            </div>

            <div className="container mx-auto max-w-[1400px] px-4 py-8">
                <ChunkDetailTopBar
                    chunkId={chunkId}
                    title={chunk.title}
                    content={chunk.content}
                    type={chunk.type}
                    isEntryPoint={isEntryPoint}
                    isAi={isAi}
                    reviewStatus={reviewStatus}
                    onArchive={() => archiveMutation.mutate()}
                    onDelete={() => deleteMutation.mutate()}
                    onSplit={() => {}}
                    onToggleEntryPoint={() => toggleEntryPointMutation.mutate()}
                    onReview={(status) => reviewMutation.mutate(status)}
                    archivePending={archiveMutation.isPending}
                    deletePending={deleteMutation.isPending}
                />

                <div className="flex gap-8">
                    <ChunkSiblingNavigator
                        currentChunkId={chunkId}
                        codebaseId={currentCodebases?.[0]?.id}
                        codebaseName={currentCodebases?.[0]?.name}
                    />

                    <ChunkDetailContent
                        chunkId={chunkId}
                        type={chunk.type}
                        title={chunk.title}
                        content={chunk.content}
                        summary={(chunk as Record<string, unknown>).summary as string | null | undefined}
                        updatedAt={chunk.updatedAt}
                        isAi={isAi}
                        reviewStatus={reviewStatus}
                        isFavorite={isFavorite(chunkId)}
                        onToggleFavorite={() => toggleFavorite(chunkId)}
                        rationale={rationale}
                        alternatives={alternatives ?? null}
                        consequences={consequences}
                        readerClasses={readerClasses}
                    />

                    <div className="max-w-[760px] mx-auto">
                        <ChunkNeighbors chunkId={chunkId} />
                    </div>

                    <ChunkProposalsSection chunkId={chunkId} />

                    <ChunkMetadataPanel
                        content={chunk.content}
                        tags={tags}
                        onTagsUpdate={(newTags) => tagMutation.mutate(newTags)}
                        tagsLoading={tagMutation.isPending}
                        type={chunk.type}
                        createdAt={chunk.createdAt}
                        updatedAt={chunk.updatedAt}
                        connectionCount={connections.length}
                        codebases={currentCodebases}
                        origin={origin}
                        reviewStatus={reviewStatus}
                        healthScore={healthScore}
                        onShowConnections={() => {
                            setDrawerTab("links");
                            setDrawerOpen(true);
                        }}
                    />
                </div>

                <button
                    type="button"
                    onClick={() => setDrawerOpen(true)}
                    className="fixed bottom-6 right-6 z-30 inline-flex items-center gap-2 rounded-full border border-indigo-500/30 bg-indigo-500/15 px-4 py-2 text-xs font-semibold text-indigo-400 shadow-lg backdrop-blur hover:bg-indigo-500/25 transition-colors print:hidden"
                    data-focus-hide="true"
                    title="More context (m)"
                >
                    ▸ More context
                    {totalSignals > 0 && (
                        <span className="text-[10px] text-indigo-400/60 font-mono">
                            ({totalSignals})
                        </span>
                    )}
                </button>

                <MoreContextDrawer
                    open={drawerOpen}
                    onOpenChange={setDrawerOpen}
                    chunkId={chunkId}
                    chunkTitle={chunk.title}
                    outgoing={outgoing}
                    incoming={incoming}
                    appliesTo={appliesTo}
                    fileReferences={fileReferences}
                    initialTab={drawerTab}
                />

            </div>
        </>
    );
}
