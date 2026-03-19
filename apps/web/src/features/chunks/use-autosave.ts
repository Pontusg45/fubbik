import { useEffect, useCallback } from "react";

const DEBOUNCE_MS = 1000;

export function useAutosave<T>(key: string, data: T, enabled = true) {
    useEffect(() => {
        if (!enabled) return;
        const timer = setTimeout(() => {
            localStorage.setItem(key, JSON.stringify(data));
        }, DEBOUNCE_MS);
        return () => clearTimeout(timer);
    }, [key, data, enabled]);

    const clearDraft = useCallback(() => {
        localStorage.removeItem(key);
    }, [key]);

    return { clearDraft };
}

export function loadDraft<T>(key: string): T | null {
    try {
        const raw = localStorage.getItem(key);
        return raw ? JSON.parse(raw) : null;
    } catch {
        return null;
    }
}
