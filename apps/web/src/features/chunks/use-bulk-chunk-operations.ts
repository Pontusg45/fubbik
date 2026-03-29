import { useCallback, useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "@/utils/api";

export function useBulkChunkOperations() {
    const queryClient = useQueryClient();
    const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
    const lastSelectedIndex = useRef<number | null>(null);

    const bulkUpdateMutation = useMutation({
        mutationFn: async (body: { ids: string[]; action: string; value?: string | null }) => {
            const { error } = await (api.api.chunks as any)["bulk-update"].post(body);
            if (error) throw new Error("Bulk update failed");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            setSelectedIds(new Set());
        }
    });

    const singleDeleteMutation = useMutation({
        mutationFn: async (id: string) => {
            const { error } = await api.api.chunks({ id }).delete();
            if (error) throw new Error("Failed to delete chunk");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            toast.success("Chunk deleted");
        },
        onError: () => {
            toast.error("Failed to delete chunk");
        }
    });

    const reviewMutation = useMutation({
        mutationFn: async ({ id, status }: { id: string; status: string }) => {
            const { error } = await api.api.chunks({ id }).patch({ reviewStatus: status as any });
            if (error) throw new Error("Failed to update review status");
        },
        onMutate: async ({ id, status }) => {
            await queryClient.cancelQueries({ queryKey: ["chunks-list"] });
            const previousQueries = queryClient.getQueriesData({ queryKey: ["chunks-list"] });
            queryClient.setQueriesData({ queryKey: ["chunks-list"] }, (old: any) => {
                if (!old?.pages) return old;
                return {
                    ...old,
                    pages: old.pages.map((page: any) => {
                        if (!page?.chunks) return page;
                        return {
                            ...page,
                            chunks: page.chunks.map((chunk: any) =>
                                chunk.id === id ? { ...chunk, reviewStatus: status } : chunk
                            ),
                        };
                    }),
                };
            });
            return { previousQueries };
        },
        onError: (_err, _vars, context) => {
            if (context?.previousQueries) {
                for (const [key, data] of context.previousQueries) {
                    queryClient.setQueryData(key, data);
                }
            }
            toast.error("Failed to update review status");
        },
        onSettled: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
        }
    });

    function toggleSelection(id: string) {
        setSelectedIds(prev => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return next;
        });
    }

    /**
     * Handle checkbox click with Shift+click range selection.
     * Pass the chunk id, its index in the displayed list, the full ordered chunk id list,
     * and the click event. If Shift is held and there is a previous selection, selects
     * all chunks in the range.
     */
    const handleSelectionClick = useCallback(
        (id: string, index: number, allIds: string[], event: React.MouseEvent) => {
            if (event.shiftKey && lastSelectedIndex.current !== null) {
                const start = Math.min(lastSelectedIndex.current, index);
                const end = Math.max(lastSelectedIndex.current, index);
                const rangeIds = allIds.slice(start, end + 1);
                setSelectedIds(prev => {
                    const next = new Set(prev);
                    for (const rid of rangeIds) {
                        next.add(rid);
                    }
                    return next;
                });
            } else {
                toggleSelection(id);
                lastSelectedIndex.current = index;
            }
        },
        []
    );

    function toggleAll(chunkIds: string[]) {
        if (selectedIds.size === chunkIds.length) {
            setSelectedIds(new Set());
        } else {
            setSelectedIds(new Set(chunkIds));
        }
        lastSelectedIndex.current = null;
    }

    return {
        selectedIds,
        setSelectedIds,
        bulkUpdateMutation,
        singleDeleteMutation,
        reviewMutation,
        toggleSelection,
        handleSelectionClick,
        toggleAll,
    };
}
