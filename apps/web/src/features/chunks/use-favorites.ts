import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function useFavorites() {
    const queryClient = useQueryClient();

    const favoritesQuery = useQuery({
        queryKey: ["favorites"],
        queryFn: async () => {
            return unwrapEden(await api.api.favorites.get());
        }
    });

    const favorites = favoritesQuery.data ?? [];
    const favoriteIds = favorites.map(f => f.chunkId);

    const addMutation = useMutation({
        mutationFn: async (chunkId: string) => {
            return unwrapEden(await api.api.favorites.post({ chunkId }));
        },
        onMutate: async (chunkId) => {
            await queryClient.cancelQueries({ queryKey: ["favorites"] });
            const previous = queryClient.getQueryData(["favorites"]);
            queryClient.setQueryData(["favorites"], (old: any) => {
                if (!Array.isArray(old)) return old;
                return [...old, { chunkId }];
            });
            return { previous };
        },
        onError: (_err, _chunkId, context) => {
            if (context?.previous) {
                queryClient.setQueryData(["favorites"], context.previous);
            }
        },
        onSettled: () => {
            queryClient.invalidateQueries({ queryKey: ["favorites"] });
        },
    });

    const removeMutation = useMutation({
        mutationFn: async (chunkId: string) => {
            return unwrapEden(await api.api.favorites({ chunkId }).delete());
        },
        onMutate: async (chunkId) => {
            await queryClient.cancelQueries({ queryKey: ["favorites"] });
            const previous = queryClient.getQueryData(["favorites"]);
            queryClient.setQueryData(["favorites"], (old: any) => {
                if (!Array.isArray(old)) return old;
                return old.filter((f: any) => f.chunkId !== chunkId);
            });
            return { previous };
        },
        onError: (_err, _chunkId, context) => {
            if (context?.previous) {
                queryClient.setQueryData(["favorites"], context.previous);
            }
        },
        onSettled: () => {
            queryClient.invalidateQueries({ queryKey: ["favorites"] });
        },
    });

    function toggleFavorite(chunkId: string) {
        if (favoriteIds.includes(chunkId)) {
            removeMutation.mutate(chunkId);
        } else {
            addMutation.mutate(chunkId);
        }
    }

    function isFavorite(chunkId: string) {
        return favoriteIds.includes(chunkId);
    }

    return { favorites, favoriteIds, toggleFavorite, isFavorite, isLoading: favoritesQuery.isLoading };
}
