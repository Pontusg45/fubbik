import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { ChevronRight, Eye, FileText, FolderOpen, Menu, Search, Tag, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { MarkdownRenderer } from "@/components/markdown-renderer";
import { Badge } from "@/components/ui/badge";
import { PageEmpty } from "@/components/ui/page";
import { Sheet, SheetContent, SheetTitle, SheetTrigger } from "@/components/ui/sheet";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { DocumentFilterBar } from "./document-filter-bar";
import { filterDocuments, groupDocuments, collectAllTags, collectAllTypes, type EnrichedDocument } from "./filter-documents";
import type { DocPresetFilters } from "./document-filter-presets";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import type { DocumentBrowserProps, DocumentDetail, DocumentListItem, SearchResult } from "./document-types";
import { buildFolderTree, extractSnippet, getStaleness } from "./document-utils";
import { highlightMatches } from "./document-highlight";
import { FolderTreeNode, IndexTree, TagGroupNode } from "./document-tree";
import { DocumentDetailView } from "./document-detail";

export function DocumentBrowser({ initialDocId, initialSection, initialGroupBy, initialTags, initialTypes }: DocumentBrowserProps) {
    const { codebaseId: activeCodebaseId } = useActiveCodebase();
    const navigate = useNavigate();
    const [selectedId, setSelectedIdState] = useState<string | null>(initialDocId ?? null);
    const [selectedGroup, setSelectedGroupState] = useState<string | null>(null);
    const [searchQuery, setSearchQuery] = useState("");
    const [isSearching, setIsSearching] = useState(false);
    const [mobileOpen, setMobileOpen] = useState(false);
    const [copiedId, setCopiedId] = useState<string | null>(null);
    const [readProgress, setReadProgress] = useState(0);
    const [highlightQuery, setHighlightQuery] = useState<string | null>(null);
    const [editingChunkId, setEditingChunkId] = useState<string | null>(null);
    const [editContent, setEditContent] = useState("");
    const [addingAfter, setAddingAfter] = useState<number | null>(null);
    const [newSectionTitle, setNewSectionTitle] = useState("");
    const [newSectionContent, setNewSectionContent] = useState("");
    const [activeTags, setActiveTags] = useState<string[]>(initialTags ?? []);
    const [activeTypes, setActiveTypes] = useState<string[]>(initialTypes ?? []);
    const [groupBy, setGroupBy] = useState<"folder" | "tag">(initialGroupBy ?? "folder");
    const queryClient = useQueryClient();

    const saveMutation = useMutation({
        mutationFn: async ({ id, content }: { id: string; content: string }) => {
            return unwrapEden(await api.api.chunks({ id }).patch({ content }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["documents", selectedId] });
            setEditingChunkId(null);
            setEditContent("");
        }
    });

    const addSectionMutation = useMutation({
        mutationFn: async ({ title, content, afterOrder }: { title: string; content: string; afterOrder: number }) => {
            if (!detail) throw new Error("No document");
            return unwrapEden(await api.api.chunks.post({
                title,
                content,
                type: "document",
                documentId: detail.id,
                documentOrder: afterOrder + 1
            }));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["documents", selectedId] });
            setAddingAfter(null);
            setNewSectionTitle("");
            setNewSectionContent("");
        }
    });

    const setSelectedId = (id: string | null) => {
        setSelectedIdState(id);
        setSelectedGroupState(null);
        navigate({
            to: "/docs",
            search: (prev: Record<string, unknown>) => ({ ...prev, id: id ?? undefined, section: undefined }),
            replace: true
        });
        window.scrollTo({ top: 0, behavior: "smooth" });
    };

    const setSelectedGroup = (name: string | null) => {
        setSelectedGroupState(name);
        setSelectedIdState(null);
        navigate({
            to: "/docs",
            search: (prev: Record<string, unknown>) => ({ ...prev, id: undefined, section: undefined }),
            replace: true
        });
        window.scrollTo({ top: 0, behavior: "smooth" });
    };

    // Fetch document list
    const listQuery = useQuery({
        queryKey: ["documents", activeCodebaseId],
        queryFn: async () => {
            try {
                const result = unwrapEden(
                    await api.api.documents.get({ query: activeCodebaseId ? { codebaseId: activeCodebaseId } : {} })
                );
                return result as DocumentListItem[];
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

    // Server-side document search — only when user types 2+ chars
    const debouncedSearch = useDebouncedValue(searchQuery, 300);
    const searchServerQuery = useQuery({
        queryKey: ["documents-search", debouncedSearch, activeCodebaseId],
        queryFn: async () => {
            try {
                const q: Record<string, string> = { q: debouncedSearch };
                if (activeCodebaseId) q.codebaseId = activeCodebaseId;
                const results = unwrapEden(
                    await api.api.documents.search.get({ query: q as any })
                ) as { chunkId: string; chunkTitle: string; chunkContent: string; documentOrder: number | null; documentId: string; documentTitle: string; sourcePath: string }[];
                return results.map(r => ({
                    documentId: r.documentId,
                    documentTitle: r.documentTitle,
                    sourcePath: r.sourcePath,
                    chunk: { id: r.chunkId, title: r.chunkTitle, content: r.chunkContent, documentOrder: r.documentOrder },
                    snippet: extractSnippet(r.chunkContent, debouncedSearch),
                })) as SearchResult[];
            } catch {
                return [];
            }
        },
        enabled: debouncedSearch.trim().length >= 2,
        staleTime: 30_000,
    });

    const documents = listQuery.data ?? [];

    const allTags = useMemo(() => collectAllTags(documents as EnrichedDocument[]), [documents]);
    const allTypes = useMemo(() => collectAllTypes(documents as EnrichedDocument[]), [documents]);

    const filteredDocuments = useMemo(
        () => filterDocuments(documents as EnrichedDocument[], { activeTags, activeTypes }),
        [documents, activeTags, activeTypes]
    );

    const groupedDocuments = useMemo(
        () => groupDocuments(filteredDocuments, groupBy),
        [filteredDocuments, groupBy]
    );

    // Fetch all document chunks for a selected tag group (combined view)
    const groupDocIds = useMemo(() => {
        if (!selectedGroup || groupBy !== "tag") return [];
        return (groupedDocuments.get(selectedGroup) ?? []).map(d => d.id);
    }, [selectedGroup, groupBy, groupedDocuments]);

    const groupDetailQuery = useQuery({
        queryKey: ["documents-group", selectedGroup, groupDocIds],
        queryFn: async () => {
            const details: DocumentDetail[] = [];
            for (const id of groupDocIds) {
                try {
                    const d = unwrapEden(await api.api.documents({ id }).get()) as DocumentDetail;
                    details.push(d);
                } catch {}
            }
            return details;
        },
        enabled: groupDocIds.length > 0,
        staleTime: 60_000,
    });

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

    // Auto-select document from URL param (but don't auto-select first doc — show index instead)
    useEffect(() => {
        if (!selectedId && documents.length > 0 && !isSearching && initialDocId) {
            if (documents.some(d => d.id === initialDocId)) {
                setSelectedIdState(initialDocId);
                navigate({
                    to: "/docs",
                    search: (prev: Record<string, unknown>) => ({ ...prev, id: initialDocId }),
                    replace: true
                });
            }
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

    // Sync filter state to URL
    useEffect(() => {
        navigate({
            to: "/docs",
            search: (prev: Record<string, unknown>) => ({
                ...prev,
                groupBy: groupBy !== "folder" ? groupBy : undefined,
                tags: activeTags.length > 0 ? activeTags.join(",") : undefined,
                types: activeTypes.length > 0 ? activeTypes.join(",") : undefined,
            }),
            replace: true,
        });
    }, [activeTags, activeTypes, groupBy]);

    // Filter action handlers
    const toggleTag = (tag: string) => {
        setActiveTags(prev => prev.includes(tag) ? prev.filter(t => t !== tag) : [...prev, tag]);
    };
    const toggleType = (type: string) => {
        setActiveTypes(prev => prev.includes(type) ? prev.filter(t => t !== type) : [...prev, type]);
    };
    const clearFilters = () => {
        setActiveTags([]);
        setActiveTypes([]);
    };
    const applyPreset = (preset: DocPresetFilters) => {
        setActiveTags(preset.activeTags);
        setActiveTypes(preset.activeTypes);
        setGroupBy(preset.groupBy);
    };

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

    // Highlight search matches in document content
    useEffect(() => {
        const content = document.querySelector("[data-doc-content]");
        if (content) {
            content.querySelectorAll("mark.search-highlight").forEach(m => {
                const parent = m.parentNode;
                if (parent) {
                    parent.replaceChild(document.createTextNode(m.textContent ?? ""), m);
                    parent.normalize();
                }
            });
        }

        if (!highlightQuery || !detail) return;
        const timer = setTimeout(() => {
            const el = document.querySelector("[data-doc-content]");
            if (!el) return;

            const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
            const matches: { node: Text; index: number }[] = [];
            const query = highlightQuery.toLowerCase();

            let node: Text | null;
            while ((node = walker.nextNode() as Text | null)) {
                const text = node.textContent?.toLowerCase() ?? "";
                let idx = text.indexOf(query);
                while (idx !== -1) {
                    matches.push({ node, index: idx });
                    idx = text.indexOf(query, idx + 1);
                }
            }

            for (const match of matches.reverse()) {
                const range = document.createRange();
                range.setStart(match.node, match.index);
                range.setEnd(match.node, match.index + highlightQuery.length);
                const mark = document.createElement("mark");
                mark.className = "search-highlight bg-yellow-200 dark:bg-yellow-800 rounded px-0.5";
                range.surroundContents(mark);
            }
        }, 500);

        return () => clearTimeout(timer);
    }, [highlightQuery, detail]);

    const searchResults = searchServerQuery.data ?? [];

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
        let docs = filteredDocuments as DocumentListItem[];
        if (searchQuery && !isSearching) {
            const q = searchQuery.toLowerCase();
            docs = docs.filter(
                d => d.title.toLowerCase().includes(q) || d.sourcePath.toLowerCase().includes(q)
            );
        }
        return docs;
    }, [filteredDocuments, searchQuery, isSearching]);

    const folderTree = useMemo(() => buildFolderTree(sidebarFiltered), [sidebarFiltered]);

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
        setHighlightQuery(searchQuery);
        setSelectedId(result.documentId);
        setIsSearching(false);
        setSearchQuery("");
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

                <DocumentFilterBar
                    allTags={allTags}
                    allTypes={allTypes}
                    activeTags={activeTags}
                    activeTypes={activeTypes}
                    groupBy={groupBy}
                    totalCount={documents.length}
                    filteredCount={filteredDocuments.length}
                    onToggleTag={toggleTag}
                    onToggleType={toggleType}
                    onSetGroupBy={setGroupBy}
                    onClearAll={clearFilters}
                    onApplyPreset={applyPreset}
                />

                {/* Search results */}
                {isSearching && (
                    <div className="max-h-[calc(100vh-280px)] space-y-1 overflow-y-auto">
                        {searchServerQuery.isLoading && (
                            <p className="text-muted-foreground px-2 py-4 text-center text-xs">Searching...</p>
                        )}
                        {!searchServerQuery.isLoading && searchResults.length === 0 && searchQuery.length >= 2 && (
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
                        <button
                            onClick={() => {
                                setSelectedIdState(null);
                                navigate({ to: "/docs", search: (prev: Record<string, unknown>) => ({ ...prev, id: undefined }), replace: true });
                                onDocSelect?.();
                            }}
                            className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors mb-2 ${
                                !selectedId ? "bg-muted text-foreground font-medium" : "text-muted-foreground hover:text-foreground hover:bg-muted/50"
                            }`}
                        >
                            <FolderOpen className="size-4" />
                            <span>All Documents</span>
                        </button>
                        {groupBy === "folder" ? (
                            <>
                                {/* Root-level docs (no folder) */}
                                {folderTree.docs.map(doc => (
                                    <button
                                        key={doc.id}
                                        onClick={() => handleDocClick(doc.id)}
                                        className={`flex w-full items-center gap-2 rounded-md px-3 py-1.5 text-left text-sm transition-colors ${
                                            selectedId === doc.id
                                                ? "bg-muted text-foreground font-medium"
                                                : "text-muted-foreground hover:text-foreground hover:bg-muted/50"
                                        }`}
                                    >
                                        <FileText className="size-3.5 shrink-0" />
                                        <span className="min-w-0 flex-1 truncate">{doc.title || doc.sourcePath.split("/").pop()}</span>
                                        <Badge variant="secondary" size="sm" className="shrink-0 font-mono text-[9px]">
                                            {doc.chunkCount}
                                        </Badge>
                                    </button>
                                ))}
                                {/* Folder tree */}
                                {folderTree.children.map(child => (
                                    <FolderTreeNode
                                        key={child.fullPath}
                                        node={child}
                                        depth={0}
                                        selectedId={selectedId}
                                        onSelect={handleDocClick}
                                        defaultOpen={folderTree.children.length <= 5}
                                    />
                                ))}
                            </>
                        ) : (
                            <div className="px-1 py-1">
                                {[...groupedDocuments.entries()].map(([groupName, groupDocs]) => (
                                    <TagGroupNode
                                        key={groupName}
                                        name={groupName}
                                        docs={groupDocs as DocumentListItem[]}
                                        selectedId={selectedId}
                                        selectedGroup={selectedGroup}
                                        onSelect={handleDocClick}
                                        onGroupSelect={(name) => { setSelectedGroup(name); onDocSelect?.(); }}
                                    />
                                ))}
                            </div>
                        )}
                    </nav>
                )}
            </div>
        );
    };

    return (
        <div className={`grid gap-8 ${showToc ? "lg:grid-cols-[240px_1fr_180px]" : "lg:grid-cols-[240px_1fr]"}`}>
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
                {/* Combined group view */}
                {selectedGroup && !selectedId && (
                    <div>
                        <div className="mb-6">
                            <div className="text-muted-foreground mb-2 flex items-center gap-1 text-xs">
                                <span>Docs</span>
                                <ChevronRight className="size-3" />
                                <span className="text-foreground font-medium flex items-center gap-1">
                                    <Tag className="size-3" />
                                    {selectedGroup}
                                </span>
                            </div>
                            <h2 className="text-xl font-bold">{selectedGroup}</h2>
                            <p className="text-muted-foreground text-sm mt-1">
                                {groupDocIds.length} document{groupDocIds.length !== 1 ? "s" : ""} in this group
                            </p>
                        </div>

                        {groupDetailQuery.isLoading && (
                            <p className="text-muted-foreground py-8 text-center text-sm">Loading documents...</p>
                        )}

                        {groupDetailQuery.data && (
                            <div className="space-y-8">
                                {groupDetailQuery.data.map(doc => (
                                    <div key={doc.id}>
                                        <div className="mb-3 flex items-center gap-2 border-b pb-2">
                                            <FileText className="text-muted-foreground size-4 shrink-0" />
                                            <h3 className="text-base font-semibold">{doc.title}</h3>
                                            <span className="text-muted-foreground text-xs font-mono">{doc.sourcePath}</span>
                                        </div>
                                        <div className="space-y-2">
                                            {doc.chunks.map(chunk => (
                                                <section key={chunk.id} id={`section-${chunk.id}`} className="scroll-mt-24">
                                                    <div className="group mb-1.5 flex items-center gap-2">
                                                        <Link
                                                            to="/chunks/$chunkId"
                                                            params={{ chunkId: chunk.id }}
                                                            className="text-lg font-semibold hover:underline underline-offset-2"
                                                        >
                                                            <h4 className="inline">{chunk.title}</h4>
                                                        </Link>
                                                        <Link
                                                            to="/chunks/$chunkId"
                                                            params={{ chunkId: chunk.id }}
                                                            className="text-muted-foreground hover:text-foreground opacity-0 transition-opacity group-hover:opacity-100"
                                                            title="Open chunk detail"
                                                        >
                                                            <Eye className="size-3.5" />
                                                        </Link>
                                                    </div>
                                                    <div className="prose prose-sm dark:prose-invert max-w-none">
                                                        <MarkdownRenderer>{chunk.content}</MarkdownRenderer>
                                                    </div>
                                                </section>
                                            ))}
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                )}

                {!selectedId && !selectedGroup && (
                    <div>
                        <h2 className="text-xl font-bold mb-6">All Documents</h2>
                        {groupBy === "folder" ? (
                            <IndexTree node={folderTree} depth={0} onSelect={setSelectedId} />
                        ) : (
                            <div className="space-y-4">
                                {[...groupedDocuments.entries()].map(([groupName, groupDocs]) => (
                                    <div key={groupName}>
                                        <h3 className="text-sm font-semibold text-muted-foreground mb-2 flex items-center gap-1.5">
                                            <Tag className="size-3.5" />
                                            {groupName}
                                        </h3>
                                        <div className="space-y-1 pl-5">
                                            {(groupDocs as DocumentListItem[]).map(doc => {
                                                const staleness = getStaleness(doc);
                                                return (
                                                    <button
                                                        key={doc.id}
                                                        onClick={() => setSelectedId(doc.id)}
                                                        className="text-foreground hover:text-foreground/80 flex items-center gap-2 text-sm w-full text-left"
                                                    >
                                                        <FileText className="size-3.5 text-muted-foreground shrink-0" />
                                                        <span>{doc.title}</span>
                                                        {doc.description && (
                                                            <span className="text-muted-foreground text-xs truncate">— {doc.description}</span>
                                                        )}
                                                        <span className={`text-xs ml-auto shrink-0 ${staleness.color}`} title={staleness.tooltip}>
                                                            {staleness.label}
                                                        </span>
                                                    </button>
                                                );
                                            })}
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                )}

                {selectedId && detailQuery.isLoading && (
                    <p className="text-muted-foreground py-8 text-center text-sm">Loading document...</p>
                )}

                {selectedId && detail && (
                    <DocumentDetailView
                        detail={detail}
                        selectedListItem={selectedListItem}
                        prevDoc={prevDoc ?? null}
                        nextDoc={nextDoc ?? null}
                        readProgress={readProgress}
                        highlightQuery={highlightQuery}
                        copiedId={copiedId}
                        editingChunkId={editingChunkId}
                        editContent={editContent}
                        addingAfter={addingAfter}
                        newSectionTitle={newSectionTitle}
                        newSectionContent={newSectionContent}
                        saveMutation={saveMutation}
                        addSectionMutation={addSectionMutation}
                        onSetHighlightQuery={setHighlightQuery}
                        onSetCopiedId={setCopiedId}
                        onSetEditingChunkId={setEditingChunkId}
                        onSetEditContent={setEditContent}
                        onSetAddingAfter={setAddingAfter}
                        onSetNewSectionTitle={setNewSectionTitle}
                        onSetNewSectionContent={setNewSectionContent}
                        onSelectDoc={setSelectedId}
                    />
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
