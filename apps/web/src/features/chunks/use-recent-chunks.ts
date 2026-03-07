import { useCallback } from "react";

import { useLocalStorage } from "@/hooks/use-local-storage";

const MAX_RECENT = 10;

export function useRecentChunks() {
    const [recentIds, setRecentIds] = useLocalStorage<string[]>("fubbik:recent", []);

    const trackView = useCallback(
        (id: string) => {
            setRecentIds(prev => {
                const filtered = prev.filter(i => i !== id);
                return [id, ...filtered].slice(0, MAX_RECENT);
            });
        },
        [setRecentIds]
    );

    return { recentIds, trackView };
}
