import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export interface CollectionFilter {
    type?: string;
    tags?: string;
    search?: string;
    sort?: string;
    after?: string;
    enrichment?: string;
    minConnections?: string;
    origin?: string;
    reviewStatus?: string;
}

export function useCollections() {
    const queryClient = useQueryClient();

    const collectionsQuery = useQuery({
        queryKey: ["collections"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.collections.get());
            } catch {
                return [];
            }
        }
    });

    const createMutation = useMutation({
        mutationFn: async (body: { name: string; description?: string; filter: CollectionFilter; codebaseId?: string }) => {
            return unwrapEden(await api.api.collections.post(body));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["collections"] });
        }
    });

    const deleteMutation = useMutation({
        mutationFn: async (id: string) => {
            return unwrapEden(await api.api.collections({ id }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["collections"] });
        }
    });

    const collections = collectionsQuery.data ?? [];

    function createCollection(name: string, filter: CollectionFilter, codebaseId?: string) {
        createMutation.mutate({ name, filter, codebaseId });
    }

    function deleteCollection(id: string) {
        deleteMutation.mutate(id);
    }

    return {
        collections,
        isLoading: collectionsQuery.isLoading,
        createCollection,
        deleteCollection
    };
}
