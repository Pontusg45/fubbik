import { useLocation, useNavigate } from "@tanstack/react-router";
import { Keyboard } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import {
    Dialog,
    DialogBackdrop,
    DialogClose,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle
} from "@/components/ui/dialog";

const SCROLL_AMOUNT = 100;

function isInputElement(target: EventTarget | null): boolean {
    if (!target || !(target instanceof HTMLElement)) return false;
    const tagName = target.tagName.toLowerCase();
    return tagName === "input" || tagName === "textarea" || tagName === "select" || target.isContentEditable;
}

// Two-key navigation combos ("go to X"). After `g`, wait up to NAV_COMBO_WINDOW
// ms for a second key; if it matches a mapping, navigate. Otherwise fall back
// to the single-`g` behaviour (scroll to top).
const NAV_COMBO_WINDOW = 800;
const NAV_COMBOS: Record<string, string> = {
    c: "/chunks",
    p: "/plans",
    r: "/requirements",
    g: "/graph",
    d: "/dashboard",
    s: "/search",
    t: "/tags",
    h: "/knowledge-health",
};

export function useGlobalShortcuts() {
    const [helpOpen, setHelpOpen] = useState(false);
    const navigate = useNavigate();
    const location = useLocation();
    // Non-reactive "are we waiting for the 2nd key of g?" — avoids stale
    // closures in the keydown handler by living in a ref.
    const gPendingRef = useRef<number | null>(null);

    const handleKeyDown = useCallback(
        (e: KeyboardEvent) => {
            if (isInputElement(e.target)) return;

            const pathname = location.pathname;

            // If we're mid-combo, treat this key as the second half.
            if (gPendingRef.current !== null) {
                const now = Date.now();
                const withinWindow = now - gPendingRef.current < NAV_COMBO_WINDOW;
                gPendingRef.current = null;
                if (withinWindow && e.key in NAV_COMBOS) {
                    e.preventDefault();
                    navigate({ to: NAV_COMBOS[e.key]! as never });
                    return;
                }
                // Otherwise let the key fall through to its normal handler below.
            }

            switch (e.key) {
                case "j":
                    e.preventDefault();
                    window.scrollBy({ top: SCROLL_AMOUNT, behavior: "smooth" });
                    break;

                case "k":
                    e.preventDefault();
                    window.scrollBy({ top: -SCROLL_AMOUNT, behavior: "smooth" });
                    break;

                case "g":
                    // Start a combo; fall back to "scroll to top" if no second
                    // key arrives within NAV_COMBO_WINDOW.
                    e.preventDefault();
                    gPendingRef.current = Date.now();
                    setTimeout(() => {
                        if (gPendingRef.current !== null) {
                            gPendingRef.current = null;
                            window.scrollTo({ top: 0, behavior: "smooth" });
                        }
                    }, NAV_COMBO_WINDOW);
                    break;

                case "G":
                    e.preventDefault();
                    window.scrollTo({ top: document.documentElement.scrollHeight, behavior: "smooth" });
                    break;

                case "f":
                    e.preventDefault();
                    document.documentElement.classList.toggle("focus-mode");
                    try {
                        localStorage.setItem(
                            "fubbik-focus-mode",
                            document.documentElement.classList.contains("focus-mode") ? "true" : "false"
                        );
                    } catch {
                        // ignore
                    }
                    break;

                case "?":
                    e.preventDefault();
                    setHelpOpen(prev => !prev);
                    break;

                case "n": {
                    e.preventDefault();
                    if (pathname.startsWith("/requirements")) {
                        navigate({ to: "/requirements/new" });
                    } else {
                        navigate({ to: "/chunks/new" });
                    }
                    break;
                }

                case "e": {
                    // Edit current item on detail pages
                    const chunkMatch = pathname.match(/^\/chunks\/([^/]+)$/);
                    const reqMatch = pathname.match(/^\/requirements\/([^/]+)$/);
                    if (chunkMatch?.[1] && chunkMatch[1] !== "new") {
                        e.preventDefault();
                        navigate({ to: "/chunks/$chunkId/edit", params: { chunkId: chunkMatch[1] } });
                    } else if (reqMatch?.[1] && reqMatch[1] !== "new") {
                        e.preventDefault();
                        navigate({ to: `/requirements/${reqMatch[1]}/edit` as string });
                    }
                    break;
                }

                case "Escape":
                    if (helpOpen) {
                        setHelpOpen(false);
                    } else {
                        window.history.back();
                    }
                    break;
            }
        },
        [location.pathname, navigate, helpOpen]
    );

    useEffect(() => {
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [handleKeyDown]);

    return { helpOpen, setHelpOpen };
}

const shortcuts = [
    { section: "Navigation", items: [
        { key: "j", description: "Scroll down" },
        { key: "k", description: "Scroll up" },
        { key: "g", description: "Jump to top (or start a 'go to' combo)" },
        { key: "G", description: "Jump to bottom" }
    ]},
    { section: "Go to", items: [
        { key: "g d", description: "Dashboard" },
        { key: "g c", description: "Chunks" },
        { key: "g g", description: "Graph" },
        { key: "g p", description: "Plans" },
        { key: "g r", description: "Requirements" },
        { key: "g s", description: "Search" },
        { key: "g t", description: "Tags" },
        { key: "g h", description: "Knowledge health" }
    ]},
    { section: "Global", items: [
        { key: "?", description: "Show keyboard shortcuts" },
        { key: "Ctrl+K", description: "Open command palette" },
        { key: "Ctrl+O", description: "Quick open chunk (fuzzy)" },
        { key: "n", description: "Create new item (context-aware)" },
        { key: "e", description: "Edit current item (on detail pages)" },
        { key: "f", description: "Toggle focus mode" },
        { key: "/", description: "Focus search" },
        { key: "Esc", description: "Go back" }
    ]},
    { section: "Chunks List", items: [
        { key: "j / k", description: "Move selection down / up" },
        { key: "Enter", description: "Open selected chunk" },
        { key: "n", description: "Create new chunk" }
    ]}
];

export function KeyboardShortcutsHelp({
    open,
    onOpenChange
}: {
    open: boolean;
    onOpenChange: (open: boolean) => void;
}) {
    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogBackdrop />
            <DialogContent className="max-w-md">
                <DialogHeader>
                    <DialogTitle className="flex items-center gap-2">
                        <Keyboard className="size-4" />
                        Keyboard Shortcuts
                    </DialogTitle>
                    <DialogDescription>Navigate quickly using these shortcuts.</DialogDescription>
                </DialogHeader>
                <div className="space-y-6 py-4">
                    {shortcuts.map(section => (
                        <div key={section.section}>
                            <h3 className="text-muted-foreground mb-2 text-xs font-semibold uppercase tracking-wide">
                                {section.section}
                            </h3>
                            <div className="space-y-2">
                                {section.items.map(item => (
                                    <div key={item.key} className="flex items-center justify-between">
                                        <span className="text-sm">{item.description}</span>
                                        <kbd className="bg-muted text-muted-foreground rounded border px-2 py-0.5 font-mono text-xs">
                                            {item.key}
                                        </kbd>
                                    </div>
                                ))}
                            </div>
                        </div>
                    ))}
                </div>
                <div className="flex justify-end">
                    <DialogClose className="text-muted-foreground hover:text-foreground text-sm">Close</DialogClose>
                </div>
            </DialogContent>
        </Dialog>
    );
}
