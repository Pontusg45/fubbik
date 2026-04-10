import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "fubbik-reading-trail";
const MAX = 15;

export interface TrailItem {
    id: string;
    title: string;
    type: string;
    visitedAt: string;
}

export function useReadingTrail() {
    const [items, setItems] = useState<TrailItem[]>([]);

    useEffect(() => {
        try {
            const stored = sessionStorage.getItem(STORAGE_KEY);
            if (stored) setItems(JSON.parse(stored));
        } catch {
            // ignore
        }
    }, []);

    const addVisit = useCallback((item: Omit<TrailItem, "visitedAt">) => {
        setItems(prev => {
            const filtered = prev.filter(i => i.id !== item.id);
            const next = [{ ...item, visitedAt: new Date().toISOString() }, ...filtered].slice(0, MAX);
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

    return { items, addVisit, clear };
}
