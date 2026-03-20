import { useEffect, useCallback, useState } from "react";

const DEBOUNCE_MS = 1000;

export function useAutosave<T>(key: string, data: T, enabled = true) {
    const [lastSaved, setLastSaved] = useState<Date | null>(null);

    useEffect(() => {
        if (!enabled) return;
        const timer = setTimeout(() => {
            localStorage.setItem(key, JSON.stringify(data));
            setLastSaved(new Date());
        }, DEBOUNCE_MS);
        return () => clearTimeout(timer);
    }, [key, data, enabled]);

    const clearDraft = useCallback(() => {
        localStorage.removeItem(key);
    }, [key]);

    return { clearDraft, lastSaved };
}

export function loadDraft<T>(key: string): T | null {
    try {
        const raw = localStorage.getItem(key);
        return raw ? JSON.parse(raw) : null;
    } catch {
        return null;
    }
}
