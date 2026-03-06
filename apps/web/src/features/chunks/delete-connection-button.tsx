import { useMutation, useQueryClient } from "@tanstack/react-query";
import { X } from "lucide-react";
import { toast } from "sonner";

import { api } from "@/utils/api";

export function DeleteConnectionButton({ connectionId, chunkId }: { connectionId: string; chunkId: string }) {
    const queryClient = useQueryClient();

    const deleteMutation = useMutation({
        mutationFn: async () => {
            const { error } = await api.api.connections({ id: connectionId }).delete();
            if (error) throw new Error("Failed to delete connection");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            toast.success("Connection removed");
        },
        onError: () => {
            toast.error("Failed to remove connection");
        }
    });

    return (
        <button
            type="button"
            onClick={e => {
                e.preventDefault();
                e.stopPropagation();
                deleteMutation.mutate();
            }}
            disabled={deleteMutation.isPending}
            className="text-muted-foreground hover:text-destructive rounded p-0.5 transition-colors"
            aria-label="Remove connection"
        >
            <X className="size-3.5" />
        </button>
    );
}
