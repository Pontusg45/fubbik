import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "fubbik-focus-mode";

export function useFocusMode() {
    const [enabled, setEnabled] = useState(false);

    useEffect(() => {
        try {
            setEnabled(localStorage.getItem(STORAGE_KEY) === "true");
        } catch {
            // ignore
        }
    }, []);

    const toggle = useCallback(() => {
        setEnabled(prev => {
            const next = !prev;
            try {
                localStorage.setItem(STORAGE_KEY, next ? "true" : "false");
            } catch {
                // ignore
            }
            return next;
        });
    }, []);

    // Apply/remove class on html element
    useEffect(() => {
        if (enabled) {
            document.documentElement.classList.add("focus-mode");
        } else {
            document.documentElement.classList.remove("focus-mode");
        }
        return () => document.documentElement.classList.remove("focus-mode");
    }, [enabled]);

    return { enabled, toggle };
}
