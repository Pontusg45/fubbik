import { useCallback } from "react";

import { useLocalStorage } from "@/hooks/use-local-storage";

export type CommandGroup =
    | "Recent"
    | "Pages"
    | "Tags"
    | "Chunks"
    | "Requirements"
    | "Plans"
    | "Codebases"
    | "Actions"
    | "All Codebases";

export interface CommandItem {
    id: string;
    title: string;
    group: CommandGroup;
    icon: React.ReactNode;
    badge?: string;
    onSelect: () => void;
}

export interface RecentPage {
    path: string;
    title: string;
    timestamp: number;
}

const MAX_RECENT_PAGES = 5;

export function useRecentPages() {
    const [pages, setPages] = useLocalStorage<RecentPage[]>("fubbik:recent-pages", []);

    const trackPage = useCallback(
        (path: string, title: string) => {
            setPages((prev) => {
                const filtered = prev.filter((p) => p.path !== path);
                return [{ path, title, timestamp: Date.now() }, ...filtered].slice(0, MAX_RECENT_PAGES);
            });
        },
        [setPages]
    );

    return { recentPages: pages, trackPage };
}
