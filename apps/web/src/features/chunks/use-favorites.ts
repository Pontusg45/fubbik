import { useLocalStorage } from "@/hooks/use-local-storage";

export function useFavorites() {
    const [favoriteIds, setFavoriteIds] = useLocalStorage<string[]>("fubbik:favorites", []);
    const favoriteSet = new Set(favoriteIds);

    function toggleFavorite(id: string) {
        setFavoriteIds(prev => (prev.includes(id) ? prev.filter(i => i !== id) : [...prev, id]));
    }

    function isFavorite(id: string) {
        return favoriteSet.has(id);
    }

    return { favoriteIds, toggleFavorite, isFavorite };
}
