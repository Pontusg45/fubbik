import { useLocalStorage } from "@/hooks/use-local-storage";

export function usePinnedChunks() {
    const [pinnedIds, setPinnedIds] = useLocalStorage<string[]>("fubbik:pinned", []);
    const pinnedSet = new Set(pinnedIds);

    function togglePin(id: string) {
        setPinnedIds(prev => (prev.includes(id) ? prev.filter(i => i !== id) : [...prev, id]));
    }

    function isPinned(id: string) {
        return pinnedSet.has(id);
    }

    return { pinnedIds, togglePin, isPinned };
}
