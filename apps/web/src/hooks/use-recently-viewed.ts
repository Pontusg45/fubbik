import { useCallback, useState } from "react";

const STORAGE_KEY = "fubbik-recently-viewed";
const MAX_ITEMS = 10;

export interface RecentItem {
    id: string;
    title: string;
    type: string;
    viewedAt: string;
}

export function useRecentlyViewed() {
    const [items, setItems] = useState<RecentItem[]>(() => {
        try {
            return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
        } catch {
            return [];
        }
    });

    const addItem = useCallback((item: Omit<RecentItem, "viewedAt">) => {
        setItems(prev => {
            const filtered = prev.filter(i => i.id !== item.id);
            const next = [{ ...item, viewedAt: new Date().toISOString() }, ...filtered].slice(0, MAX_ITEMS);
            localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
            return next;
        });
    }, []);

    return { items, addItem };
}
