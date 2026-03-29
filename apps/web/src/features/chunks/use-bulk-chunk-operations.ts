import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "@/utils/api";

export function useBulkChunkOperations() {
    const queryClient = useQueryClient();
    const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

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
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
        },
        onError: () => {
            toast.error("Failed to update review status");
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

    function toggleAll(chunkIds: string[]) {
        if (selectedIds.size === chunkIds.length) {
            setSelectedIds(new Set());
        } else {
            setSelectedIds(new Set(chunkIds));
        }
    }

    return {
        selectedIds,
        setSelectedIds,
        bulkUpdateMutation,
        singleDeleteMutation,
        reviewMutation,
        toggleSelection,
        toggleAll,
    };
}
