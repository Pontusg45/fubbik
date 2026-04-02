import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { FileText, FolderOpen, Pencil, Search } from "lucide-react";
import { useMemo, useState } from "react";

import { MarkdownRenderer } from "@/components/markdown-renderer";
import { Badge } from "@/components/ui/badge";
import { PageEmpty } from "@/components/ui/page";
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

/* ─── Helpers ─── */

function folderFromPath(sourcePath: string): string {
    const parts = sourcePath.split("/");
    if (parts.length <= 1) return "/";
    return parts.slice(0, -1).join("/");
}

function filenameFromPath(sourcePath: string): string {
    return sourcePath.split("/").pop() ?? sourcePath;
}

/* ─── Document Browser ─── */

export function DocumentBrowser() {
    const { codebaseId } = useActiveCodebase();
    const [selectedId, setSelectedId] = useState<string | null>(null);
    const [searchFilter, setSearchFilter] = useState("");

    // Fetch document list (show all documents, not filtered by codebase)
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

    const documents = listQuery.data ?? [];

    // Filter and group by folder
    const filtered = useMemo(() => {
        if (!searchFilter) return documents;
        const q = searchFilter.toLowerCase();
        return documents.filter(
            d => d.title.toLowerCase().includes(q) || d.sourcePath.toLowerCase().includes(q)
        );
    }, [documents, searchFilter]);

    const grouped = useMemo(() => {
        const map = new Map<string, DocumentListItem[]>();
        for (const doc of filtered) {
            const folder = folderFromPath(doc.sourcePath);
            const list = map.get(folder) ?? [];
            list.push(doc);
            map.set(folder, list);
        }
        return Array.from(map.entries()).sort(([a], [b]) => a.localeCompare(b));
    }, [filtered]);

    const detail = detailQuery.data;

    if (listQuery.isLoading) {
        return <p className="text-muted-foreground py-8 text-center text-sm">Loading documents...</p>;
    }

    if (documents.length === 0) {
        return (
            <PageEmpty
                icon={FileText}
                title="No documents"
                description="Import markdown files to create browsable documents. Use the CLI: fubbik import docs/"
            />
        );
    }

    return (
        <div className="grid gap-6 lg:grid-cols-[280px_1fr]">
            {/* ─── Sidebar ─── */}
            <div className="space-y-3">
                {/* Search */}
                <div className="relative">
                    <Search className="text-muted-foreground absolute left-2.5 top-2.5 size-4" />
                    <input
                        type="text"
                        placeholder="Filter documents..."
                        value={searchFilter}
                        onChange={e => setSearchFilter(e.target.value)}
                        className="border-input bg-background placeholder:text-muted-foreground w-full rounded-md border py-2 pl-9 pr-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                    />
                </div>

                {/* Folder-grouped list */}
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
                                        onClick={() => setSelectedId(doc.id)}
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
            </div>

            {/* ─── Main content ─── */}
            <div className="min-w-0">
                {!selectedId && (
                    <div className="flex flex-col items-center gap-3 py-16">
                        <FileText className="text-muted-foreground/30 size-10" />
                        <p className="text-muted-foreground text-sm">Select a document from the sidebar to view it.</p>
                    </div>
                )}

                {selectedId && detailQuery.isLoading && (
                    <p className="text-muted-foreground py-8 text-center text-sm">Loading document...</p>
                )}

                {selectedId && detail && (
                    <div>
                        {/* Document header */}
                        <div className="mb-6">
                            <h2 className="text-xl font-bold">{detail.title}</h2>
                            {detail.description && (
                                <p className="text-muted-foreground mt-1 text-sm">{detail.description}</p>
                            )}
                            <p className="text-muted-foreground mt-1 font-mono text-xs">{detail.sourcePath}</p>
                        </div>

                        {/* On this page nav (for 3+ sections) */}
                        {detail.chunks.length >= 3 && (
                            <div className="border-border mb-6 rounded-lg border p-4">
                                <p className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">On this page</p>
                                <ul className="space-y-1">
                                    {detail.chunks.map(chunk => (
                                        <li key={chunk.id}>
                                            <a
                                                href={`#section-${chunk.id}`}
                                                className="text-muted-foreground hover:text-foreground text-sm transition-colors"
                                            >
                                                {chunk.title}
                                            </a>
                                        </li>
                                    ))}
                                </ul>
                            </div>
                        )}

                        {/* Sections */}
                        <div className="space-y-8">
                            {detail.chunks.map(chunk => (
                                <section key={chunk.id} id={`section-${chunk.id}`} className="scroll-mt-24">
                                    <div className="group mb-3 flex items-center gap-2">
                                        <h3 className="text-lg font-semibold">{chunk.title}</h3>
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
                    </div>
                )}
            </div>
        </div>
    );
}
