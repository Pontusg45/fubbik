import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "fubbik-recent-queries";
const MAX_ITEMS = 15;

export interface RecentQuery {
    q: string;
    usedAt: string;
}

export function useRecentQueries() {
    const [items, setItems] = useState<RecentQuery[]>([]);

    useEffect(() => {
        try {
            const stored = sessionStorage.getItem(STORAGE_KEY);
            if (stored) setItems(JSON.parse(stored));
        } catch {
            // ignore
        }
    }, []);

    const addQuery = useCallback((q: string) => {
        if (!q.trim()) return;
        setItems(prev => {
            const filtered = prev.filter(i => i.q !== q);
            const next = [{ q, usedAt: new Date().toISOString() }, ...filtered].slice(0, MAX_ITEMS);
            try {
                sessionStorage.setItem(STORAGE_KEY, JSON.stringify(next));
            } catch {
                // ignore
            }
            return next;
        });
    }, []);

    const clear = useCallback(() => {
        setItems([]);
        try {
            sessionStorage.removeItem(STORAGE_KEY);
        } catch {
            // ignore
        }
    }, []);

    return { items, addQuery, clear };
}
