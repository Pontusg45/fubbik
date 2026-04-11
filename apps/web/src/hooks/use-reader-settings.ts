import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "fubbik-reader-settings";

export interface ReaderSettings {
    fontSize: "sm" | "base" | "lg" | "xl";
    lineHeight: "tight" | "normal" | "relaxed";
    maxWidth: "narrow" | "normal" | "wide";
}

const DEFAULTS: ReaderSettings = {
    fontSize: "base",
    lineHeight: "normal",
    maxWidth: "normal",
};

export function useReaderSettings() {
    const [settings, setSettings] = useState<ReaderSettings>(DEFAULTS);

    useEffect(() => {
        try {
            const stored = localStorage.getItem(STORAGE_KEY);
            if (stored) setSettings({ ...DEFAULTS, ...JSON.parse(stored) });
        } catch {
            // ignore
        }
    }, []);

    const update = useCallback((partial: Partial<ReaderSettings>) => {
        setSettings(prev => {
            const next = { ...prev, ...partial };
            try {
                localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
            } catch {
                // ignore
            }
            return next;
        });
    }, []);

    return { settings, update };
}

export function getReaderClasses(settings: ReaderSettings): string {
    const fontSize = {
        sm: "text-sm",
        base: "text-base",
        lg: "text-lg",
        xl: "text-xl",
    }[settings.fontSize];
    const lineHeight = {
        tight: "leading-tight",
        normal: "leading-relaxed",
        relaxed: "leading-loose",
    }[settings.lineHeight];
    const maxWidth = {
        narrow: "max-w-2xl",
        normal: "max-w-3xl",
        wide: "max-w-5xl",
    }[settings.maxWidth];
    return `${fontSize} ${lineHeight} ${maxWidth}`;
}
