import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { getChunkSize } from "@/features/chunks/chunk-size";
import { usePinnedChunks } from "@/features/chunks/use-pinned-chunks";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

const LIMIT = 20;

export function useChunksData({
    type,
    q,
    sort,
    tags,
    size,
    after,
    enrichment,
    minConnections,
    codebaseId,
    origin,
    reviewStatus,
    isFederated,
}: {
    type?: string;
    q?: string;
    sort?: string;
    tags?: string;
    size?: string;
    after?: string;
    enrichment?: string;
    minConnections?: string;
    codebaseId?: string | null;
    origin?: string;
    reviewStatus?: string;
    isFederated: boolean;
}) {
    const queryClient = useQueryClient();

    const chunksQuery = useInfiniteQuery({
        queryKey: ["chunks-list", type, q, sort, tags, after, enrichment, minConnections, codebaseId, origin, reviewStatus],
        queryFn: async ({ pageParam = 1 }) => {
            try {
                return unwrapEden(
                    await api.api.chunks.get({
                        query: {
                            type,
                            search: q,
                            sort: sort as "newest" | "oldest" | "alpha" | "updated" | undefined,
                            tags,
                            after,
                            enrichment: enrichment as "missing" | "complete" | undefined,
                            minConnections,
                            limit: String(LIMIT),
                            offset: String((pageParam - 1) * LIMIT),
                            ...(codebaseId === "global" ? { global: "true" } : codebaseId ? { codebaseId } : {}),
                            origin: origin as "human" | "ai" | undefined,
                            reviewStatus: reviewStatus as "draft" | "reviewed" | "approved" | undefined
                        }
                    })
                );
            } catch {
                return null;
            }
        },
        enabled: !isFederated,
        initialPageParam: 1,
        getNextPageParam: (lastPage, allPages) => {
            if (!lastPage) return undefined;
            const loaded = allPages.reduce((sum, p) => sum + (p?.chunks?.length ?? 0), 0);
            return loaded < lastPage.total ? allPages.length + 1 : undefined;
        }
    });

    const federatedQuery = useInfiniteQuery({
        queryKey: ["chunks-federated", type, q, sort, tags, origin, reviewStatus],
        queryFn: async ({ pageParam = 1 }) => {
            try {
                const res = await api.api.chunks.search.federated.get({
                    query: {
                        type,
                        search: q,
                        sort: sort as "newest" | "oldest" | "alpha" | "updated" | undefined,
                        tags,
                        limit: String(LIMIT),
                        offset: String((pageParam - 1) * LIMIT),
                    }
                });
                return unwrapEden(res);
            } catch {
                return null;
            }
        },
        enabled: isFederated,
        initialPageParam: 1,
        getNextPageParam: (lastPage, allPages) => {
            if (!lastPage) return undefined;
            const loaded = allPages.reduce((sum, p) => sum + (p?.chunks?.length ?? 0), 0);
            return loaded < lastPage.total ? allPages.length + 1 : undefined;
        }
    });

    const tagsQuery = useQuery({
        queryKey: ["tags"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.tags.get());
            } catch {
                return [];
            }
        },
        staleTime: 60_000
    });

    const activeQuery = isFederated ? federatedQuery : chunksQuery;
    const allChunks = activeQuery.data?.pages.flatMap(p => p?.chunks ?? []) ?? [];

    const { pinnedIds, togglePin, isPinned } = usePinnedChunks();

    const processedChunks = useMemo(() => {
        const filtered = size ? allChunks.filter(c => getChunkSize(c.content).level === size) : allChunks;
        const pinnedSet = new Set(pinnedIds);
        return [...filtered].sort((a, b) => {
            const aPinned = pinnedSet.has(a.id) ? 0 : 1;
            const bPinned = pinnedSet.has(b.id) ? 0 : 1;
            return aPinned - bPinned;
        });
        // eslint-disable-next-line react-hooks/exhaustive-deps -- allChunks is stable per fetch
    }, [activeQuery.data, size, pinnedIds]);

    // Inline title editing
    const [editingChunkId, setEditingChunkId] = useState<string | null>(null);
    const [editTitle, setEditTitle] = useState("");

    const editMutation = useMutation({
        mutationFn: async ({ id, title }: { id: string; title: string }) =>
            unwrapEden(await api.api.chunks({ id }).patch({ title })),
        onSuccess: () => queryClient.invalidateQueries({ queryKey: ["chunks-list"] })
    });

    const startEditing = (chunkId: string, currentTitle: string) => {
        setEditingChunkId(chunkId);
        setEditTitle(currentTitle);
    };

    const commitEdit = () => {
        if (editingChunkId && editTitle.trim() && editTitle.trim() !== "") {
            editMutation.mutate({ id: editingChunkId, title: editTitle.trim() });
        }
        setEditingChunkId(null);
    };

    const cancelEdit = () => {
        setEditingChunkId(null);
    };

    // Prefetch chunk detail on hover
    const handleChunkHover = (chunkId: string) => {
        queryClient.prefetchQuery({
            queryKey: ["chunk", chunkId],
            queryFn: async () => unwrapEden(await api.api.chunks({ id: chunkId }).get()),
            staleTime: 30_000
        });
    };

    return {
        chunksQuery,
        federatedQuery,
        activeQuery,
        tagsQuery,
        allChunks,
        processedChunks,
        // inline editing
        editingChunkId,
        editTitle,
        setEditTitle,
        startEditing,
        commitEdit,
        cancelEdit,
        // hover prefetch
        handleChunkHover,
        // pinning
        togglePin,
        isPinned,
    };
}
