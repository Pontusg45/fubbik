import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export interface CollectionFilter {
    type?: string | null;
    tags?: string | null;
    search?: string | null;
    sort?: string | null;
    after?: string | null;
    enrichment?: string | null;
    minConnections?: string | null;
    origin?: string | null;
    reviewStatus?: string | null;
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
        mutationFn: async (body: { name: string; description?: string; filter: CollectionFilter; spaceId?: string }) => {
            // Rust's `CollectionFilter` requires all nine keys present (value
            // `string | null`, never an absent key / `undefined`) — the
            // client's tightened body type now enforces that. Normalize the
            // caller's sparse, possibly-`undefined`-valued filter into the
            // exact wire shape instead of relying on `JSON.stringify` to drop
            // `undefined` keys (which happened to produce an equivalent
            // payload, but wasn't type-checked before).
            const { filter, ...rest } = body;
            return unwrapEden(
                await api.api.collections.post({
                    ...rest,
                    filter: {
                        type: filter.type ?? null,
                        tags: filter.tags ?? null,
                        search: filter.search ?? null,
                        sort: filter.sort ?? null,
                        after: filter.after ?? null,
                        enrichment: filter.enrichment ?? null,
                        minConnections: filter.minConnections ?? null,
                        origin: filter.origin ?? null,
                        reviewStatus: filter.reviewStatus ?? null
                    }
                })
            );
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

    function createCollection(name: string, filter: CollectionFilter, spaceId?: string) {
        createMutation.mutate({ name, filter, spaceId });
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
