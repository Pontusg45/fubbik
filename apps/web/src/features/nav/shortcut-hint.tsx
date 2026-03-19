import { useState } from "react";
import { X } from "lucide-react";

const DISMISSED_KEY = "fubbik-shortcut-hint-dismissed";

export function ShortcutHint() {
    const [dismissed, setDismissed] = useState(() => {
        if (typeof window === "undefined") return true;
        return localStorage.getItem(DISMISSED_KEY) === "true";
    });

    if (dismissed) return null;

    const dismiss = () => {
        setDismissed(true);
        localStorage.setItem(DISMISSED_KEY, "true");
    };

    return (
        <div className="text-muted-foreground flex items-center gap-2 text-xs">
            <span>
                Press <kbd className="bg-muted rounded border px-1 font-mono text-[10px]">/</kbd> to search,{" "}
                <kbd className="bg-muted rounded border px-1 font-mono text-[10px]">j</kbd>/
                <kbd className="bg-muted rounded border px-1 font-mono text-[10px]">k</kbd> to navigate,{" "}
                <kbd className="bg-muted rounded border px-1 font-mono text-[10px]">?</kbd> for all shortcuts
            </span>
            <button onClick={dismiss} className="hover:text-foreground">
                <X className="size-3" />
            </button>
        </div>
    );
}
