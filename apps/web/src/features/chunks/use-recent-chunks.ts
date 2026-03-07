import { useLocalStorage } from "@/hooks/use-local-storage";

const MAX_RECENT = 10;

export function useRecentChunks() {
    const [recentIds, setRecentIds] = useLocalStorage<string[]>("fubbik:recent", []);

    function trackView(id: string) {
        setRecentIds(prev => {
            const filtered = prev.filter(i => i !== id);
            return [id, ...filtered].slice(0, MAX_RECENT);
        });
    }

    return { recentIds, trackView };
}
