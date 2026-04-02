import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { Check, ChevronLeft, ChevronRight, FileText, FolderOpen, Link2, Menu, Pencil, Search, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { MarkdownRenderer } from "@/components/markdown-renderer";
import { Badge } from "@/components/ui/badge";
import { PageEmpty } from "@/components/ui/page";
import { Sheet, SheetContent, SheetTitle, SheetTrigger } from "@/components/ui/sheet";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

/* ─── Types ─── */

interface DocumentListItem {
    id: string;
    title: string;
    sourcePath: string;
    description: string | null;
    chunkCount: number;
    updatedAt: string;
}

interface DocumentChunk {
    id: string;
    title: string;
    content: string;
    documentOrder: number | null;
}

interface DocumentDetail {
    id: string;
    title: string;
    sourcePath: string;
    description: string | null;
    chunks: DocumentChunk[];
}

interface SearchResult {
    documentId: string;
    documentTitle: string;
    sourcePath: string;
    chunk: DocumentChunk;
    snippet: string;
}

/* ─── Helpers ─── */

function folderFromPath(sourcePath: string): string {
    const parts = sourcePath.split("/");
    if (parts.length <= 1) return "/";
    return parts.slice(0, -1).join("/");
}

function filenameFromPath(sourcePath: string): string {
    return sourcePath.split("/").pop() ?? sourcePath;
}

function extractSnippet(content: string, query: string, contextChars = 120): string {
    const lower = content.toLowerCase();
    const idx = lower.indexOf(query.toLowerCase());
    if (idx === -1) return content.slice(0, contextChars * 2) + (content.length > contextChars * 2 ? "..." : "");

    const start = Math.max(0, idx - contextChars);
    const end = Math.min(content.length, idx + query.length + contextChars);
    let snippet = content.slice(start, end);
    if (start > 0) snippet = "..." + snippet;
    if (end < content.length) snippet = snippet + "...";
    return snippet;
}

function highlightMatches(text: string, query: string): React.ReactNode[] {
    if (!query) return [text];
    const regex = new RegExp(`(${query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "gi");
    const parts = text.split(regex);
    return parts.map((part, i) =>
        regex.test(part) ? (
            <mark key={i} className="bg-yellow-200 dark:bg-yellow-800 rounded px-0.5">
                {part}
            </mark>
        ) : (
            part
        )
    );
}

function estimateReadingTime(chunks: DocumentChunk[]): number {
    const words = chunks.reduce((sum, c) => sum + c.content.split(/\s+/).length, 0);
    return Math.max(1, Math.ceil(words / 200));
}

/* ─── Document Browser ─── */

interface DocumentBrowserProps {
    initialDocId?: string;
    initialSection?: string;
}

export function DocumentBrowser({ initialDocId, initialSection }: DocumentBrowserProps) {
    const { codebaseId } = useActiveCodebase();
    const navigate = useNavigate();
    const [selectedId, setSelectedIdState] = useState<string | null>(initialDocId ?? null);
    const [searchQuery, setSearchQuery] = useState("");
    const [isSearching, setIsSearching] = useState(false);
    const [mobileOpen, setMobileOpen] = useState(false);
    const [copiedId, setCopiedId] = useState<string | null>(null);
    const [readProgress, setReadProgress] = useState(0);

    const setSelectedId = (id: string | null) => {
        setSelectedIdState(id);
        navigate({
            to: "/docs",
            search: (prev: Record<string, unknown>) => ({ ...prev, id: id ?? undefined, section: undefined }),
            replace: true
        });
        window.scrollTo({ top: 0, behavior: "smooth" });
    };

    // Fetch document list
    const listQuery = useQuery({
        queryKey: ["documents"],
        queryFn: async () => {
            try {
                return unwrapEden(
                    await api.api.documents.get({ query: {} })
                ) as DocumentListItem[];
            } catch {
                return [];
            }
        }
    });

    // Fetch selected document detail
    const detailQuery = useQuery({
        queryKey: ["documents", selectedId],
        queryFn: async () => {
            if (!selectedId) return null;
            try {
                return unwrapEden(
                    await api.api.documents({ id: selectedId }).get()
                ) as DocumentDetail;
            } catch {
                return null;
            }
        },
        enabled: !!selectedId
    });

    // Fetch all document details for search (only when searching)
    const allDocsQuery = useQuery({
        queryKey: ["documents-all-details"],
        queryFn: async () => {
            const docs = listQuery.data ?? [];
            const details: DocumentDetail[] = [];
            for (const doc of docs) {
                try {
                    const detail = unwrapEden(
                        await api.api.documents({ id: doc.id }).get()
                    ) as DocumentDetail;
                    details.push(detail);
                } catch {
                    // skip failed fetches
                }
            }
            return details;
        },
        enabled: isSearching && (listQuery.data?.length ?? 0) > 0,
        staleTime: 60_000
    });

    const documents = listQuery.data ?? [];
    const detail = detailQuery.data;
    const selectedListItem = documents.find(d => d.id === selectedId);

    const currentIndex = documents.findIndex(d => d.id === selectedId);
    const prevDoc = currentIndex > 0 ? documents[currentIndex - 1] : null;
    const nextDoc = currentIndex < documents.length - 1 ? documents[currentIndex + 1] : null;

    const showToc = !!detail && detail.chunks.length >= 3;

    const [activeSection, setActiveSection] = useState<string | null>(null);

    useEffect(() => {
        if (!detail) return;
        const observer = new IntersectionObserver(
            entries => {
                for (const entry of entries) {
                    if (entry.isIntersecting) {
                        setActiveSection(entry.target.id);
                    }
                }
            },
            { rootMargin: "-80px 0px -70% 0px", threshold: 0 }
        );
        const sections = document.querySelectorAll("[id^='section-']");
        sections.forEach(s => observer.observe(s));
        return () => observer.disconnect();
    }, [detail]);

    // Auto-select first document when none is selected
    useEffect(() => {
        if (!selectedId && documents.length > 0 && !isSearching) {
            const firstId = initialDocId && documents.some(d => d.id === initialDocId) ? initialDocId : documents[0]!.id;
            setSelectedIdState(firstId);
            navigate({
                to: "/docs",
                search: (prev: Record<string, unknown>) => ({ ...prev, id: firstId }),
                replace: true
            });
        }
    }, [documents]);

    // Scroll to section from URL on detail load
    useEffect(() => {
        if (initialSection && detail) {
            const el = document.getElementById(`section-${initialSection}`);
            el?.scrollIntoView({ behavior: "smooth", block: "start" });
        }
    }, [initialSection, detail]);

    // Keyboard navigation
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            const target = e.target as HTMLElement;
            if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable) return;

            if (e.key === "/" && !e.metaKey && !e.ctrlKey) {
                e.preventDefault();
                const input = document.querySelector<HTMLInputElement>("[data-docs-search]");
                input?.focus();
                return;
            }

            if (e.key === "Escape") {
                if (isSearching) { clearSearch(); return; }
                const input = document.querySelector<HTMLInputElement>("[data-docs-search]");
                input?.blur();
                return;
            }

            if (e.key === "ArrowUp" || e.key === "k") {
                e.preventDefault();
                if (prevDoc) setSelectedId(prevDoc.id);
                return;
            }

            if (e.key === "ArrowDown" || e.key === "j") {
                e.preventDefault();
                if (nextDoc) setSelectedId(nextDoc.id);
                return;
            }
        };

        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [prevDoc, nextDoc, isSearching]);

    // Reading progress
    useEffect(() => {
        if (!detail) return;
        const handleScroll = () => {
            const scrollTop = window.scrollY;
            const docHeight = document.documentElement.scrollHeight - window.innerHeight;
            if (docHeight <= 0) { setReadProgress(100); return; }
            setReadProgress(Math.min(100, Math.round((scrollTop / docHeight) * 100)));
        };
        window.addEventListener("scroll", handleScroll, { passive: true });
        handleScroll();
        return () => window.removeEventListener("scroll", handleScroll);
    }, [detail]);

    // Full-text search across all documents
    const searchResults = useMemo((): SearchResult[] => {
        if (!searchQuery.trim() || !allDocsQuery.data) return [];
        const q = searchQuery.toLowerCase();
        const results: SearchResult[] = [];

        for (const doc of allDocsQuery.data) {
            for (const chunk of doc.chunks) {
                const inTitle = chunk.title.toLowerCase().includes(q);
                const inContent = chunk.content.toLowerCase().includes(q);
                if (inTitle || inContent) {
                    results.push({
                        documentId: doc.id,
                        documentTitle: doc.title,
                        sourcePath: doc.sourcePath,
                        chunk,
                        snippet: extractSnippet(chunk.content, searchQuery)
                    });
                }
            }
        }

        // Also match document titles
        for (const doc of allDocsQuery.data) {
            if (doc.title.toLowerCase().includes(q) && !results.some(r => r.documentId === doc.id)) {
                const firstChunk = doc.chunks[0];
                if (firstChunk) {
                    results.push({
                        documentId: doc.id,
                        documentTitle: doc.title,
                        sourcePath: doc.sourcePath,
                        chunk: firstChunk,
                        snippet: firstChunk.content.slice(0, 200) + (firstChunk.content.length > 200 ? "..." : "")
                    });
                }
            }
        }

        return results;
    }, [searchQuery, allDocsQuery.data]);

    const groupedSearchResults = useMemo(() => {
        const map = new Map<string, { doc: { id: string; title: string }; results: SearchResult[] }>();
        for (const result of searchResults) {
            const existing = map.get(result.documentId);
            if (existing) {
                existing.results.push(result);
            } else {
                map.set(result.documentId, {
                    doc: { id: result.documentId, title: result.documentTitle },
                    results: [result]
                });
            }
        }
        return Array.from(map.values());
    }, [searchResults]);

    // Sidebar filtering (by document title/path when not in full search mode)
    const sidebarFiltered = useMemo(() => {
        if (!searchQuery || isSearching) return documents;
        const q = searchQuery.toLowerCase();
        return documents.filter(
            d => d.title.toLowerCase().includes(q) || d.sourcePath.toLowerCase().includes(q)
        );
    }, [documents, searchQuery, isSearching]);

    const grouped = useMemo(() => {
        const map = new Map<string, DocumentListItem[]>();
        for (const doc of sidebarFiltered) {
            const folder = folderFromPath(doc.sourcePath);
            const list = map.get(folder) ?? [];
            list.push(doc);
            map.set(folder, list);
        }
        for (const [, docs] of map) {
            docs.sort((a, b) => a.title.localeCompare(b.title));
        }
        return Array.from(map.entries()).sort(([a], [b]) => a.localeCompare(b));
    }, [sidebarFiltered]);

    const handleSearch = (value: string) => {
        setSearchQuery(value);
        if (value.trim().length >= 2) {
            setIsSearching(true);
        } else {
            setIsSearching(false);
        }
    };

    const clearSearch = () => {
        setSearchQuery("");
        setIsSearching(false);
    };

    const navigateToResult = (result: SearchResult) => {
        setSelectedId(result.documentId);
        setIsSearching(false);
        setSearchQuery("");
        // Scroll to section after a brief delay for the detail to load
        setTimeout(() => {
            const el = document.getElementById(`section-${result.chunk.id}`);
            el?.scrollIntoView({ behavior: "smooth", block: "start" });
        }, 300);
    };

    if (listQuery.isLoading) {
        return <p className="text-muted-foreground py-8 text-center text-sm">Loading documents...</p>;
    }

    if (documents.length === 0) {
        return (
            <PageEmpty
                icon={FileText}
                title="No documents"
                description="Import markdown files to create browsable documents. Use the CLI: fubbik docs import-dir docs/"
            />
        );
    }

    const renderSidebar = (onDocSelect?: () => void) => {
        const handleDocClick = (id: string) => {
            setSelectedId(id);
            onDocSelect?.();
        };
        return (
            <div className="space-y-3">
                {/* Search */}
                <div className="relative">
                    <Search className="text-muted-foreground absolute left-2.5 top-2.5 size-4" />
                    <input
                        type="text"
                        data-docs-search
                        placeholder="Search across all docs..."
                        value={searchQuery}
                        onChange={e => handleSearch(e.target.value)}
                        className="border-input bg-background placeholder:text-muted-foreground w-full rounded-md border py-2 pl-9 pr-8 text-sm outline-none focus:ring-2 focus:ring-ring"
                    />
                    {searchQuery && (
                        <button
                            onClick={clearSearch}
                            className="text-muted-foreground hover:text-foreground absolute right-2.5 top-2.5"
                        >
                            <X className="size-4" />
                        </button>
                    )}
                </div>

                {/* Search results */}
                {isSearching && (
                    <div className="max-h-[calc(100vh-280px)] space-y-1 overflow-y-auto">
                        {allDocsQuery.isLoading && (
                            <p className="text-muted-foreground px-2 py-4 text-center text-xs">Searching...</p>
                        )}
                        {!allDocsQuery.isLoading && searchResults.length === 0 && searchQuery.length >= 2 && (
                            <p className="text-muted-foreground px-2 py-4 text-center text-xs">No results for "{searchQuery}"</p>
                        )}
                        {groupedSearchResults.map(group => (
                            <div key={group.doc.id} className="mb-3">
                                <div className="flex items-center gap-1.5 px-2 py-1">
                                    <FileText className="text-muted-foreground size-3.5" />
                                    <span className="text-xs font-semibold">{group.doc.title}</span>
                                    <Badge variant="secondary" size="sm" className="ml-auto text-[9px]">
                                        {group.results.length}
                                    </Badge>
                                </div>
                                {group.results.map((result, i) => (
                                    <button
                                        key={`${result.chunk.id}-${i}`}
                                        onClick={() => {
                                            navigateToResult(result);
                                            onDocSelect?.();
                                        }}
                                        className="hover:bg-muted/50 w-full rounded-md px-3 py-2 text-left transition-colors"
                                    >
                                        <p className="text-sm font-medium">{highlightMatches(result.chunk.title, searchQuery)}</p>
                                        <p className="text-muted-foreground mt-0.5 line-clamp-2 text-xs">
                                            {highlightMatches(result.snippet, searchQuery)}
                                        </p>
                                    </button>
                                ))}
                            </div>
                        ))}
                    </div>
                )}

                {/* Folder-grouped list (hidden during search) */}
                {!isSearching && (
                    <nav className="max-h-[calc(100vh-280px)] space-y-4 overflow-y-auto">
                        {grouped.map(([folder, docs]) => (
                            <div key={folder}>
                                <div className="text-muted-foreground mb-1 flex items-center gap-1.5 px-2 text-xs font-medium">
                                    <FolderOpen className="size-3.5" />
                                    <span className="truncate">{folder || "/"}</span>
                                </div>
                                <div className="space-y-0.5">
                                    {docs.map(doc => (
                                        <button
                                            key={doc.id}
                                            onClick={() => handleDocClick(doc.id)}
                                            className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors ${
                                                selectedId === doc.id
                                                    ? "bg-muted text-foreground font-medium"
                                                    : "text-muted-foreground hover:text-foreground hover:bg-muted/50"
                                            }`}
                                        >
                                            <FileText className="size-4 shrink-0" />
                                            <span className="min-w-0 flex-1 truncate">{doc.title || filenameFromPath(doc.sourcePath)}</span>
                                            <Badge variant="secondary" size="sm" className="shrink-0 font-mono text-[9px]">
                                                {doc.chunkCount}
                                            </Badge>
                                        </button>
                                    ))}
                                </div>
                            </div>
                        ))}
                    </nav>
                )}
            </div>
        );
    };

    return (
        <div className={`grid gap-6 ${showToc ? "lg:grid-cols-[280px_1fr_200px]" : "lg:grid-cols-[280px_1fr]"}`}>
            {/* ─── Sidebar ─── */}
            {/* Desktop sidebar */}
            <div className="hidden lg:block">{renderSidebar()}</div>

            {/* Mobile trigger + sheet */}
            <div className="lg:hidden">
                <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
                    <SheetTrigger
                        render={
                            <button className="border-input bg-background flex w-full items-center gap-2 rounded-md border px-3 py-2 text-sm">
                                <Menu className="size-4" />
                                <span className="text-muted-foreground truncate">{detail?.title ?? "Select document..."}</span>
                            </button>
                        }
                    />
                    <SheetContent side="left" className="w-80 p-4">
                        <SheetTitle className="mb-4 text-sm font-semibold">Documents</SheetTitle>
                        {renderSidebar(() => setMobileOpen(false))}
                    </SheetContent>
                </Sheet>
            </div>

            {/* ─── Main content ─── */}
            <div className="min-w-0">
                {selectedId && detail && (
                    <div className="bg-muted mb-4 h-0.5 w-full overflow-hidden rounded-full">
                        <div
                            className="bg-foreground/30 h-full transition-all duration-150"
                            style={{ width: `${readProgress}%` }}
                        />
                    </div>
                )}

                {selectedId && detailQuery.isLoading && (
                    <p className="text-muted-foreground py-8 text-center text-sm">Loading document...</p>
                )}

                {selectedId && detail && (
                    <div>
                        {/* Document header */}
                        <div className="mb-6">
                            <div className="text-muted-foreground mb-2 flex items-center gap-1 text-xs">
                                <span>Docs</span>
                                {folderFromPath(detail.sourcePath) !== "/" && (
                                    <>
                                        <ChevronRight className="size-3" />
                                        <span>{folderFromPath(detail.sourcePath)}</span>
                                    </>
                                )}
                                <ChevronRight className="size-3" />
                                <span className="text-foreground font-medium">{detail.title}</span>
                            </div>
                            <h2 className="text-xl font-bold">{detail.title}</h2>
                            {detail.description && (
                                <p className="text-muted-foreground mt-1 text-sm">{detail.description}</p>
                            )}
                            <p className="text-muted-foreground mt-1 font-mono text-xs">{detail.sourcePath}</p>
                            {selectedListItem?.updatedAt && (
                                <p className="text-muted-foreground mt-1 text-xs">
                                    Last updated {new Date(selectedListItem.updatedAt).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" })}
                                </p>
                            )}
                            <p className="text-muted-foreground mt-1 text-xs">
                                ~{estimateReadingTime(detail.chunks)} min read
                            </p>
                        </div>

                        {/* Sections */}
                        <div className="space-y-8">
                            {detail.chunks.map(chunk => (
                                <section key={chunk.id} id={`section-${chunk.id}`} className="scroll-mt-24">
                                    <div className="group mb-3 flex items-center gap-2">
                                        <h3 className="text-lg font-semibold">{chunk.title}</h3>
                                        <button
                                            onClick={() => {
                                                const url = `${window.location.origin}/docs?id=${detail.id}&section=${chunk.id}`;
                                                navigator.clipboard.writeText(url);
                                                setCopiedId(chunk.id);
                                                setTimeout(() => setCopiedId(null), 1500);
                                            }}
                                            className="text-muted-foreground hover:text-foreground opacity-0 transition-opacity group-hover:opacity-100"
                                            title="Copy link to section"
                                        >
                                            {copiedId === chunk.id ? (
                                                <Check className="size-3.5 text-green-500" />
                                            ) : (
                                                <Link2 className="size-3.5" />
                                            )}
                                        </button>
                                        <Link
                                            to="/chunks/$chunkId/edit"
                                            params={{ chunkId: chunk.id }}
                                            className="text-muted-foreground hover:text-foreground opacity-0 transition-opacity group-hover:opacity-100"
                                            title="Edit this section"
                                        >
                                            <Pencil className="size-3.5" />
                                        </Link>
                                    </div>
                                    <div className="prose prose-sm dark:prose-invert max-w-none">
                                        <MarkdownRenderer>{chunk.content}</MarkdownRenderer>
                                    </div>
                                </section>
                            ))}
                        </div>

                        {(prevDoc || nextDoc) && (
                            <div className="border-border mt-10 flex items-center justify-between border-t pt-6">
                                {prevDoc ? (
                                    <button
                                        onClick={() => setSelectedId(prevDoc.id)}
                                        className="text-muted-foreground hover:text-foreground group flex items-center gap-2 text-sm transition-colors"
                                    >
                                        <ChevronLeft className="size-4 transition-transform group-hover:-translate-x-0.5" />
                                        <div className="text-left">
                                            <p className="text-xs text-muted-foreground">Previous</p>
                                            <p className="font-medium">{prevDoc.title}</p>
                                        </div>
                                    </button>
                                ) : <div />}
                                {nextDoc ? (
                                    <button
                                        onClick={() => setSelectedId(nextDoc.id)}
                                        className="text-muted-foreground hover:text-foreground group flex items-center gap-2 text-sm transition-colors"
                                    >
                                        <div className="text-right">
                                            <p className="text-xs text-muted-foreground">Next</p>
                                            <p className="font-medium">{nextDoc.title}</p>
                                        </div>
                                        <ChevronRight className="size-4 transition-transform group-hover:translate-x-0.5" />
                                    </button>
                                ) : <div />}
                            </div>
                        )}
                    </div>
                )}
            </div>

            {showToc && (
                <nav className="hidden lg:block">
                    <div className="sticky top-24 space-y-2">
                        <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">On this page</p>
                        <ul className="space-y-1 border-l border-border pl-3">
                            {detail.chunks.map(chunk => (
                                <li key={chunk.id}>
                                    <a
                                        href={`#section-${chunk.id}`}
                                        className={`block text-xs leading-relaxed transition-colors ${
                                            activeSection === `section-${chunk.id}`
                                                ? "text-foreground font-medium"
                                                : "text-muted-foreground hover:text-foreground"
                                        }`}
                                    >
                                        {chunk.title}
                                    </a>
                                </li>
                            ))}
                        </ul>
                    </div>
                </nav>
            )}
        </div>
    );
}
