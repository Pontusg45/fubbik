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
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["favorites"] });
        }
    });

    const removeMutation = useMutation({
        mutationFn: async (chunkId: string) => {
            return unwrapEden(await api.api.favorites({ chunkId }).delete());
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["favorites"] });
        }
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
