import { useMutation, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
    Activity,
    Blocks,
    BookOpen,
    ClipboardCheck,
    Clock,
    FileCode,
    FileText,
    Globe,
    Hash,
    LayoutDashboard,
    ListChecks,
    Network,
    Plus,
    Search,
    Server,
    Settings,
    Tags,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Kbd } from "@/components/ui/kbd";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { useRecentChunks } from "@/features/chunks/use-recent-chunks";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { useLocalStorage } from "@/hooks/use-local-storage";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

type CommandGroup =
    | "Recent"
    | "Pages"
    | "Tags"
    | "Chunks"
    | "Requirements"
    | "Plans"
    | "Codebases"
    | "Actions"
    | "All Codebases";

interface CommandItem {
    id: string;
    title: string;
    group: CommandGroup;
    icon: React.ReactNode;
    badge?: string;
    onSelect: () => void;
}

// --- Recent pages tracking ---
interface RecentPage {
    path: string;
    title: string;
    timestamp: number;
}

const MAX_RECENT_PAGES = 5;

export function useRecentPages() {
    const [pages, setPages] = useLocalStorage<RecentPage[]>("fubbik:recent-pages", []);

    const trackPage = useCallback(
        (path: string, title: string) => {
            setPages((prev) => {
                const filtered = prev.filter((p) => p.path !== path);
                return [{ path, title, timestamp: Date.now() }, ...filtered].slice(0, MAX_RECENT_PAGES);
            });
        },
        [setPages]
    );

    return { recentPages: pages, trackPage };
}

const PAGE_ITEMS: Array<{ id: string; title: string; path: string; icon: React.ReactNode }> = [
    { id: "page-dashboard", title: "Dashboard", path: "/dashboard", icon: <LayoutDashboard className="size-4" /> },
    { id: "page-chunks", title: "Chunks", path: "/chunks", icon: <Blocks className="size-4" /> },
    { id: "page-graph", title: "Graph", path: "/graph", icon: <Network className="size-4" /> },
    { id: "page-tags", title: "Tags", path: "/tags", icon: <Tags className="size-4" /> },
    { id: "page-codebases", title: "Codebases", path: "/codebases", icon: <Server className="size-4" /> },
    { id: "page-templates", title: "Templates", path: "/templates", icon: <FileCode className="size-4" /> },
    { id: "page-health", title: "Health", path: "/knowledge-health", icon: <Activity className="size-4" /> },
    { id: "page-requirements", title: "Requirements", path: "/requirements", icon: <FileText className="size-4" /> },
    { id: "page-vocabulary", title: "Vocabulary", path: "/vocabulary", icon: <BookOpen className="size-4" /> },
];

const ACTION_ITEMS: Array<{
    id: string;
    title: string;
    path: string;
    search?: Record<string, string>;
    icon: React.ReactNode;
    /** If set, this action triggers a sub-mode instead of navigating */
    subMode?: string;
}> = [
    { id: "action-new-chunk", title: "New Chunk", path: "/chunks/new", icon: <Plus className="size-4" /> },
    { id: "action-new-note", title: "New Note", path: "/chunks/new", search: { type: "note" }, icon: <FileText className="size-4" /> },
    { id: "action-new-document", title: "New Document", path: "/chunks/new", search: { type: "document" }, icon: <FileText className="size-4" /> },
    { id: "action-new-requirement", title: "New Requirement", path: "/requirements/new", icon: <Plus className="size-4" /> },
    { id: "action-new-plan", title: "New Plan", path: "/plans/new", icon: <Plus className="size-4" /> },
    { id: "action-switch-codebase", title: "Switch Codebase", path: "", icon: <Settings className="size-4" />, subMode: "codebase" },
    { id: "action-view-health", title: "View Health", path: "/knowledge-health", icon: <Activity className="size-4" /> },
];

export function CommandPalette() {
    const [open, setOpen] = useState(false);
    const [query, setQuery] = useState("");
    const [subMode, setSubMode] = useState<string | null>(null);
    const [selectedIndex, setSelectedIndex] = useState(0);
    const inputRef = useRef<HTMLInputElement>(null);
    const listRef = useRef<HTMLDivElement>(null);
    const navigate = useNavigate();

    const debouncedQuery = useDebouncedValue(query, 200);
    const { recentIds } = useRecentChunks();
    const { recentPages } = useRecentPages();
    const { setCodebaseId } = useActiveCodebase();

    const quickNoteMutation = useMutation({
        mutationFn: async (title: string) => {
            const result = unwrapEden(
                await api.api.chunks.post({
                    title,
                    content: "",
                    type: "note",
                })
            );
            return result;
        },
        onSuccess: (data) => {
            const chunk = data as { id: string };
            toast.success(`Created "${query.trim()}"`, {
                action: {
                    label: "Edit",
                    onClick: () => navigate({ to: "/chunks/$chunkId/edit", params: { chunkId: chunk.id } }),
                },
            });
            close();
        },
        onError: () => {
            toast.error("Failed to create note");
        },
    });

    const isTagSearch = query.startsWith("#");
    const tagQuery = isTagSearch ? query.slice(1).toLowerCase() : "";
    const isFederatedSearch = query.startsWith("*");
    const federatedQuery = isFederatedSearch ? query.slice(1).trim() : "";

    // Global Cmd+K / Ctrl+K shortcut
    useEffect(() => {
        const down = (e: KeyboardEvent) => {
            if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                setOpen((prev) => !prev);
            }
        };
        document.addEventListener("keydown", down);
        return () => document.removeEventListener("keydown", down);
    }, []);

    // Reset state when closing
    const close = useCallback(() => {
        setOpen(false);
        setQuery("");
        setSubMode(null);
        setSelectedIndex(0);
    }, []);

    const debouncedFederatedQuery = useDebouncedValue(federatedQuery, 200);

    // Search chunks via API
    const chunkSearch = useQuery({
        queryKey: ["command-palette-chunks", debouncedQuery],
        queryFn: async () => {
            if (!debouncedQuery.trim()) return null;
            try {
                return unwrapEden(
                    await api.api.chunks.get({
                        query: { search: debouncedQuery, limit: "5" },
                    })
                );
            } catch {
                return null;
            }
        },
        enabled: open && debouncedQuery.length > 1 && !isTagSearch && !isFederatedSearch,
    });

    // Federated search (across all codebases) when query starts with *
    const federatedSearch = useQuery({
        queryKey: ["command-palette-federated", debouncedFederatedQuery],
        queryFn: async () => {
            if (!debouncedFederatedQuery.trim()) return null;
            try {
                return unwrapEden(
                    await api.api.chunks.search.federated.get({
                        query: { search: debouncedFederatedQuery, limit: "8" },
                    })
                );
            } catch {
                return null;
            }
        },
        enabled: open && isFederatedSearch && debouncedFederatedQuery.length > 0,
    });

    // Search tags via API when query starts with #
    const tagSearch = useQuery({
        queryKey: ["command-palette-tags"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.tags.get()) as Array<{
                    id: string;
                    name: string;
                    tagTypeName: string | null;
                    tagTypeColor: string | null;
                }>;
            } catch {
                return [];
            }
        },
        enabled: open && isTagSearch,
    });

    // Search requirements via API
    const requirementsSearch = useQuery({
        queryKey: ["command-palette-requirements", debouncedQuery],
        queryFn: async () => {
            if (!debouncedQuery.trim()) return null;
            try {
                return unwrapEden(
                    await api.api.requirements.get({
                        query: { search: debouncedQuery, limit: "5" },
                    })
                ) as { requirements: Array<{ id: string; title: string; status: string; priority: string }>; total: number };
            } catch {
                return null;
            }
        },
        enabled: open && debouncedQuery.length > 1 && !isTagSearch && !isFederatedSearch && subMode === null,
    });

    // Search plans via API (fetch all and filter client-side since API doesn't support search param)
    const plansSearch = useQuery({
        queryKey: ["command-palette-plans"],
        queryFn: async () => {
            try {
                return unwrapEden(
                    await api.api.plans.get({ query: {} })
                ) as Array<{ id: string; title: string; status: string }>;
            } catch {
                return [];
            }
        },
        enabled: open && debouncedQuery.length > 1 && !isTagSearch && !isFederatedSearch && subMode === null,
        staleTime: 30_000,
    });

    // Fetch codebases for Switch Codebase sub-mode
    const codebasesQuery = useQuery({
        queryKey: ["command-palette-codebases"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.codebases.get()) as Array<{
                    id: string;
                    name: string;
                    remoteUrl: string | null;
                }>;
            } catch {
                return [];
            }
        },
        enabled: open && subMode === "codebase",
        staleTime: 60_000,
    });

    // Fetch recent chunk details
    const recentChunksQuery = useQuery({
        queryKey: ["command-palette-recent", recentIds],
        queryFn: async () => {
            if (recentIds.length === 0) return [];
            try {
                const results = await Promise.all(
                    recentIds.slice(0, 5).map(async (id) => {
                        try {
                            const data = unwrapEden(
                                await api.api.chunks({ id }).get()
                            );
                            return data;
                        } catch {
                            return null;
                        }
                    })
                );
                return results.filter(Boolean);
            } catch {
                return [];
            }
        },
        enabled: open && !query.trim() && recentIds.length > 0,
    });

    // Build flattened items list
    const items = useMemo(() => {
        const lowerQuery = query.toLowerCase();
        const result: CommandItem[] = [];

        // Sub-mode: codebase switcher
        if (subMode === "codebase") {
            const codebases = codebasesQuery.data ?? [];
            const filtered = codebases.filter(
                (c) => !lowerQuery || c.name.toLowerCase().includes(lowerQuery)
            );
            for (const cb of filtered) {
                result.push({
                    id: `cb-${cb.id}`,
                    title: cb.name,
                    group: "Codebases",
                    icon: <Server className="size-4" />,
                    badge: cb.remoteUrl ? "git" : undefined,
                    onSelect: () => {
                        setCodebaseId(cb.id);
                        close();
                    },
                });
            }
            return result;
        }

        // Federated search mode: when query starts with *
        if (isFederatedSearch) {
            const chunks = federatedSearch.data?.chunks ?? [];
            for (const chunk of chunks) {
                const cName = (chunk as Record<string, unknown>).codebaseName as string | null;
                result.push({
                    id: `fed-${chunk.id}`,
                    title: chunk.title ?? `Chunk ${chunk.id.slice(0, 8)}`,
                    group: "All Codebases",
                    icon: <Globe className="size-4" />,
                    badge: cName ?? "Global",
                    onSelect: () => {
                        navigate({ to: "/chunks/$chunkId", params: { chunkId: chunk.id } });
                        close();
                    },
                });
            }
            return result;
        }

        // Tag search mode: when query starts with #
        if (isTagSearch) {
            const tags = tagSearch.data ?? [];
            const filtered = tags.filter(
                (t) => !tagQuery || t.name.toLowerCase().includes(tagQuery)
            );
            for (const tag of filtered.slice(0, 10)) {
                result.push({
                    id: `tag-${tag.id}`,
                    title: `#${tag.name}`,
                    group: "Tags",
                    icon: <Hash className="size-4" />,
                    onSelect: () => {
                        navigate({ to: "/chunks", search: { tags: tag.name } });
                        close();
                    },
                });
            }
            return result;
        }

        // Recent items (shown when query is empty)
        if (!query.trim()) {
            // Recent pages first
            for (const page of recentPages) {
                result.push({
                    id: `recent-page-${page.path}`,
                    title: page.title,
                    group: "Recent",
                    icon: <Clock className="size-4" />,
                    badge: "Page",
                    onSelect: () => {
                        navigate({ to: page.path });
                        close();
                    },
                });
            }

            // Recent chunks
            const recentChunks = recentChunksQuery.data ?? [];
            for (const item of recentChunks) {
                if (!item) continue;
                const c = item.chunk;
                result.push({
                    id: `recent-${c.id}`,
                    title: c.title ?? `Chunk ${c.id.slice(0, 8)}`,
                    group: "Recent",
                    icon: <Clock className="size-4" />,
                    onSelect: () => {
                        navigate({ to: "/chunks/$chunkId", params: { chunkId: c.id } });
                        close();
                    },
                });
            }
        }

        // Pages (filtered by query)
        const filteredPages = PAGE_ITEMS.filter(
            (p) => !lowerQuery || p.title.toLowerCase().includes(lowerQuery)
        );
        for (const page of filteredPages) {
            result.push({
                id: page.id,
                title: page.title,
                group: "Pages",
                icon: page.icon,
                onSelect: () => {
                    navigate({ to: page.path });
                    close();
                },
            });
        }

        // Chunks from API search
        const chunks = chunkSearch.data?.chunks ?? [];
        for (const chunk of chunks) {
            result.push({
                id: `chunk-${chunk.id}`,
                title: chunk.title ?? `Chunk ${chunk.id.slice(0, 8)}`,
                group: "Chunks",
                icon: <Blocks className="size-4" />,
                onSelect: () => {
                    navigate({ to: "/chunks/$chunkId", params: { chunkId: chunk.id } });
                    close();
                },
            });
        }

        // Requirements from API search
        if (debouncedQuery.length > 1) {
            const requirements = requirementsSearch.data?.requirements ?? [];
            for (const req of requirements.slice(0, 5)) {
                result.push({
                    id: `req-${req.id}`,
                    title: req.title,
                    group: "Requirements",
                    icon: <ClipboardCheck className="size-4" />,
                    badge: req.status,
                    onSelect: () => {
                        navigate({ to: "/requirements/$requirementId", params: { requirementId: req.id } });
                        close();
                    },
                });
            }

            // Plans (client-side filtered)
            const allPlans = Array.isArray(plansSearch.data) ? plansSearch.data : [];
            const filteredPlans = allPlans
                .filter((p) => p.title.toLowerCase().includes(lowerQuery))
                .slice(0, 5);
            for (const plan of filteredPlans) {
                result.push({
                    id: `plan-${plan.id}`,
                    title: plan.title,
                    group: "Plans",
                    icon: <ListChecks className="size-4" />,
                    badge: plan.status,
                    onSelect: () => {
                        navigate({ to: "/plans/$planId", params: { planId: plan.id } });
                        close();
                    },
                });
            }
        }

        // Actions (filtered by query)
        const filteredActions = ACTION_ITEMS.filter(
            (a) => !lowerQuery || a.title.toLowerCase().includes(lowerQuery)
        );
        for (const action of filteredActions) {
            result.push({
                id: action.id,
                title: action.title,
                group: "Actions",
                icon: action.icon,
                onSelect: () => {
                    if (action.subMode) {
                        setSubMode(action.subMode);
                        setQuery("");
                        setSelectedIndex(0);
                    } else {
                        navigate({ to: action.path, search: action.search ?? {} });
                        close();
                    }
                },
            });
        }

        if (query.trim().length > 0 && !isTagSearch && !isFederatedSearch && !subMode) {
            result.push({
                id: "quick-note",
                title: `Quick note: "${query.trim()}"`,
                group: "Actions",
                icon: <Plus className="size-4" />,
                badge: "Create",
                onSelect: () => quickNoteMutation.mutate(query.trim()),
            });
        }

        return result;
    }, [query, debouncedQuery, subMode, isTagSearch, isFederatedSearch, tagQuery, tagSearch.data, federatedSearch.data, recentPages, recentChunksQuery.data, chunkSearch.data, requirementsSearch.data, plansSearch.data, codebasesQuery.data, navigate, close, setCodebaseId, quickNoteMutation]);

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

    // Group items for rendering
    const groups = new Map<string, { items: CommandItem[]; startIndex: number }>();
    let idx = 0;
    for (const item of items) {
        if (!groups.has(item.group)) {
            groups.set(item.group, { items: [], startIndex: idx });
        }
        groups.get(item.group)!.items.push(item);
        idx++;
    }

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
                                {subMode === "codebase" ? "Switch Codebase" : subMode}
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
