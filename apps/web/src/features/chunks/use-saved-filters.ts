import { useLocalStorage } from "@/hooks/use-local-storage";

export interface SavedFilter {
    name: string;
    params: Record<string, string | undefined>;
}

export function useSavedFilters() {
    const [filters, setFilters] = useLocalStorage<SavedFilter[]>("fubbik:saved-filters", []);

    function saveFilter(name: string, params: Record<string, string | undefined>) {
        setFilters(prev => [...prev.filter(f => f.name !== name), { name, params }]);
    }

    function deleteFilter(name: string) {
        setFilters(prev => prev.filter(f => f.name !== name));
    }

    return { filters, saveFilter, deleteFilter };
}
