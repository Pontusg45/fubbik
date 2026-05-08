import { useNavigate } from "@tanstack/react-router";
import { Search } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Kbd } from "@/components/ui/kbd";

import { groupItems } from "./command-items";
import { useRecentPages } from "./command-types";
import { useCommandSearch } from "./use-command-search";

export { useRecentPages };

export function CommandPalette() {
    const [open, setOpen] = useState(false);
    const [query, setQuery] = useState("");
    const [subMode, setSubMode] = useState<string | null>(null);
    const [selectedIndex, setSelectedIndex] = useState(0);
    const inputRef = useRef<HTMLInputElement>(null);
    const listRef = useRef<HTMLDivElement>(null);
    const navigate = useNavigate();

    const { recentPages } = useRecentPages();

    // Reset state when closing
    const close = useCallback(() => {
        setOpen(false);
        setQuery("");
        setSubMode(null);
        setSelectedIndex(0);
    }, []);

    const { items } = useCommandSearch({
        open,
        query,
        subMode,
        recentPages,
        close,
        setSubMode,
        setQuery,
        setSelectedIndex,
    });

    // Global Cmd+K / Ctrl+K shortcut, and Ctrl+O / Cmd+O for chunk quick-open
    useEffect(() => {
        const down = (e: KeyboardEvent) => {
            if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                setOpen((prev) => !prev);
            } else if (e.key === "o" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                setOpen(true);
                setSubMode("chunks");
                setQuery("");
                setSelectedIndex(0);
            }
        };
        document.addEventListener("keydown", down);
        return () => document.removeEventListener("keydown", down);
    }, []);

    // Clamp selected index when items change
    useEffect(() => {
        setSelectedIndex((prev) => Math.min(prev, Math.max(0, items.length - 1)));
    }, [items.length]);

    // Scroll selected item into view
    useEffect(() => {
        if (!listRef.current) return;
        const selectedEl = listRef.current.querySelector(`[data-index="${selectedIndex}"]`);
        selectedEl?.scrollIntoView({ block: "nearest" });
    }, [selectedIndex]);

    // Keyboard navigation
    const handleKeyDown = useCallback(
        (e: React.KeyboardEvent) => {
            if (e.key === "ArrowDown") {
                e.preventDefault();
                setSelectedIndex((prev) => (prev + 1) % items.length);
            } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setSelectedIndex((prev) => (prev - 1 + items.length) % items.length);
            } else if (e.key === "Enter") {
                e.preventDefault();
                items[selectedIndex]?.onSelect();
            } else if (e.key === "Escape") {
                e.preventDefault();
                if (subMode) {
                    setSubMode(null);
                    setQuery("");
                    setSelectedIndex(0);
                } else {
                    close();
                }
            } else if (e.key === "Backspace" && query === "" && subMode) {
                e.preventDefault();
                setSubMode(null);
            }
        },
        [items, selectedIndex, close, subMode, query]
    );

    if (!open) return null;

    const groups = groupItems(items);

    return (
        <>
            {/* Backdrop */}
            <div
                className="fixed inset-0 z-50 bg-black/32 backdrop-blur-sm"
                onClick={close}
                onKeyDown={(e) => {
                    if (e.key === "Escape") close();
                }}
            />

            {/* Panel */}
            <div
                className="fixed inset-x-0 top-0 z-50 flex justify-center px-4 pt-[max(1rem,10vh)]"
                onKeyDown={handleKeyDown}
            >
                <div className="w-full max-w-xl overflow-hidden rounded-2xl border bg-popover shadow-lg/5">
                    {/* Search input */}
                    <div className="flex items-center gap-2 border-b px-4 py-3">
                        <Search className="text-muted-foreground size-4 shrink-0" />
                        {subMode && (
                            <Badge variant="secondary" size="sm" className="shrink-0">
                                {subMode === "codebase" ? "Switch Codebase" : subMode === "chunks" ? "Go to Chunk" : subMode}
                            </Badge>
                        )}
                        <input
                            ref={inputRef}
                            type="text"
                            value={query}
                            onChange={(e) => {
                                setQuery(e.target.value);
                                setSelectedIndex(0);
                            }}
                            placeholder={
                                subMode === "codebase"
                                    ? "Filter codebases..."
                                    : subMode === "chunks"
                                      ? "Fuzzy search chunks..."
                                      : "Type a command or search... (# tags, * all codebases)"
                            }
                            autoFocus
                            className="placeholder:text-muted-foreground flex-1 bg-transparent text-sm outline-none"
                        />
                        <Kbd>Esc</Kbd>
                    </div>

                    {/* Results */}
                    <div ref={listRef} className="max-h-80 overflow-y-auto p-2">
                        {items.length === 0 && (
                            <p className="text-muted-foreground py-6 text-center text-sm">
                                No results found.
                            </p>
                        )}

                        {Array.from(groups.entries()).map(([groupName, group]) => (
                            <div key={groupName} className="mb-1">
                                <p className="px-2 py-1.5 font-medium text-muted-foreground text-xs">
                                    {groupName}
                                </p>
                                {group.items.map((item, i) => {
                                    const globalIndex = group.startIndex + i;
                                    return (
                                        <button
                                            key={item.id}
                                            type="button"
                                            data-index={globalIndex}
                                            className={`flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm transition-colors ${
                                                globalIndex === selectedIndex
                                                    ? "bg-accent text-accent-foreground"
                                                    : "text-foreground hover:bg-muted"
                                            }`}
                                            onClick={item.onSelect}
                                            onMouseEnter={() => setSelectedIndex(globalIndex)}
                                        >
                                            <span className="text-muted-foreground shrink-0">
                                                {item.icon}
                                            </span>
                                            <span className="min-w-0 flex-1 truncate">
                                                {item.title}
                                            </span>
                                            {item.badge ? (
                                                <Badge variant="outline" size="sm" className="border-blue-500/30 bg-blue-500/10 text-blue-600">
                                                    {item.badge}
                                                </Badge>
                                            ) : (
                                                <Badge variant="secondary" size="sm">
                                                    {item.group === "All Codebases"
                                                        ? "Global"
                                                        : item.group === "Codebases"
                                                          ? "Codebase"
                                                          : item.group === "Actions"
                                                            ? "Action"
                                                            : item.group}
                                                </Badge>
                                            )}
                                        </button>
                                    );
                                })}
                            </div>
                        ))}
                    </div>

                    {/* Footer */}
                    <div className="flex items-center gap-4 border-t px-4 py-2 text-muted-foreground text-xs">
                        {query.trim().length >= 2 && (
                            <button
                                type="button"
                                className="text-primary hover:underline"
                                onClick={() => {
                                    navigate({ to: "/search", search: { q: query.trim() } });
                                    close();
                                }}
                            >
                                See all results
                            </button>
                        )}
                        <span className="flex-1" />
                        <span className="flex items-center gap-1">
                            <Kbd>↑↓</Kbd> Navigate
                        </span>
                        <span className="flex items-center gap-1">
                            <Kbd>↵</Kbd> Select
                        </span>
                        <span className="flex items-center gap-1">
                            <Kbd>Esc</Kbd> Close
                        </span>
                    </div>
                </div>
            </div>
        </>
    );
}
