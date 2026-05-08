import { Link } from "@tanstack/react-router";
import { Check, ChevronLeft, ChevronRight, Eye, Link2, Pencil, Plus, Printer, X } from "lucide-react";
import { Fragment } from "react";
import type { UseMutationResult } from "@tanstack/react-query";

import { MarkdownRenderer } from "@/components/markdown-renderer";
import { estimateReadingTime, folderFromPath, getStaleness, mdToHtml } from "./document-utils";
import type { DocumentChunk, DocumentDetail, DocumentListItem } from "./document-types";

export interface DocumentDetailViewProps {
    detail: DocumentDetail;
    selectedListItem: DocumentListItem | undefined;
    prevDoc: DocumentListItem | null;
    nextDoc: DocumentListItem | null;
    readProgress: number;
    highlightQuery: string | null;
    copiedId: string | null;
    editingChunkId: string | null;
    editContent: string;
    addingAfter: number | null;
    newSectionTitle: string;
    newSectionContent: string;
    saveMutation: UseMutationResult<unknown, Error, { id: string; content: string }>;
    addSectionMutation: UseMutationResult<unknown, Error, { title: string; content: string; afterOrder: number }>;
    onSetHighlightQuery: (query: string | null) => void;
    onSetCopiedId: (id: string | null) => void;
    onSetEditingChunkId: (id: string | null) => void;
    onSetEditContent: (content: string) => void;
    onSetAddingAfter: (order: number | null) => void;
    onSetNewSectionTitle: (title: string) => void;
    onSetNewSectionContent: (content: string) => void;
    onSelectDoc: (id: string) => void;
}

export function DocumentDetailView({
    detail,
    selectedListItem,
    prevDoc,
    nextDoc,
    readProgress,
    highlightQuery,
    copiedId,
    editingChunkId,
    editContent,
    addingAfter,
    newSectionTitle,
    newSectionContent,
    saveMutation,
    addSectionMutation,
    onSetHighlightQuery,
    onSetCopiedId,
    onSetEditingChunkId,
    onSetEditContent,
    onSetAddingAfter,
    onSetNewSectionTitle,
    onSetNewSectionContent,
    onSelectDoc,
}: DocumentDetailViewProps) {
    const handlePrint = () => {
        const printWindow = window.open("", "_blank");
        if (!printWindow) return;
        const chunks = detail.chunks;
        const html = `<!DOCTYPE html>
<html>
<head>
    <title>${detail.title}</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 700px; margin: 40px auto; padding: 0 20px; line-height: 1.6; color: #1a1a1a; }
        h1 { font-size: 28px; margin-bottom: 8px; }
        h2 { font-size: 20px; margin-top: 32px; border-bottom: 1px solid #e5e7eb; padding-bottom: 8px; }
        h3 { font-size: 16px; margin-top: 24px; }
        code { background: #f3f4f6; padding: 2px 6px; border-radius: 4px; font-size: 14px; }
        pre { background: #f3f4f6; padding: 16px; border-radius: 8px; overflow-x: auto; }
        pre code { background: none; padding: 0; }
        table { border-collapse: collapse; width: 100%; margin: 16px 0; }
        th, td { border: 1px solid #e5e7eb; padding: 8px 12px; text-align: left; font-size: 14px; }
        th { background: #f9fafb; }
        li { margin: 4px 0; }
        .meta { color: #6b7280; font-size: 14px; margin-bottom: 32px; }
        @media print { body { margin: 0; } }
    </style>
</head>
<body>
    <h1>${detail.title}</h1>
    <p class="meta">${detail.sourcePath}</p>
    ${chunks.map(c => {
        const isIntro = c.title.includes("-- Introduction") || c.title.endsWith(" Introduction");
        const contentHtml = mdToHtml(c.content);
        return isIntro ? contentHtml : `<h2>${c.title}</h2>\n${contentHtml}`;
    }).join("\n\n")}
</body>
</html>`;
        printWindow.document.write(html);
        printWindow.document.close();
        printWindow.print();
    };

    return (
        <div>
            {/* Reading progress bar */}
            <div className="bg-muted mb-4 h-0.5 w-full overflow-hidden rounded-full">
                <div
                    className="bg-foreground/30 h-full transition-all duration-150"
                    style={{ width: `${readProgress}%` }}
                />
            </div>

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
                <div className="flex items-center gap-3">
                    <h2 className="text-xl font-bold">{detail.title}</h2>
                    <button
                        onClick={handlePrint}
                        className="text-muted-foreground hover:text-foreground transition-colors"
                        title="Print document"
                    >
                        <Printer className="size-4" />
                    </button>
                </div>
                {detail.description && (
                    <p className="text-muted-foreground mt-1 text-sm">{detail.description}</p>
                )}
                <p className="text-muted-foreground mt-1 font-mono text-xs">{detail.sourcePath}</p>
                {selectedListItem && (() => {
                    const staleness = getStaleness(selectedListItem);
                    const contentDate = selectedListItem.lastChunkUpdatedAt ?? selectedListItem.updatedAt;
                    return (
                        <p className="text-muted-foreground mt-1 flex items-center gap-2 text-xs" title={staleness.tooltip}>
                            Last updated {new Date(contentDate).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" })}
                            <span className={`font-medium ${staleness.color}`}>{staleness.label}</span>
                        </p>
                    );
                })()}
                <p className="text-muted-foreground mt-1 text-xs">
                    ~{estimateReadingTime(detail.chunks)} min read
                </p>
            </div>

            {/* Search highlight banner */}
            {highlightQuery && (
                <div className="bg-muted mb-4 flex items-center justify-between rounded-md px-3 py-2 text-sm">
                    <span className="text-muted-foreground">Showing results for "<strong>{highlightQuery}</strong>"</span>
                    <button onClick={() => onSetHighlightQuery(null)} className="text-muted-foreground hover:text-foreground">
                        <X className="size-4" />
                    </button>
                </div>
            )}

            {/* Sections */}
            <div className="space-y-2" data-doc-content>
                {detail.chunks.map((chunk, idx) => (
                    <Fragment key={chunk.id}>
                        <section id={`section-${chunk.id}`} className="scroll-mt-24">
                            <div className="group mb-1.5 flex items-center gap-2">
                                <Link
                                    to="/chunks/$chunkId"
                                    params={{ chunkId: chunk.id }}
                                    className="text-lg font-semibold hover:underline underline-offset-2"
                                >
                                    <h3 className="inline">{chunk.title}</h3>
                                </Link>
                                <Link
                                    to="/chunks/$chunkId"
                                    params={{ chunkId: chunk.id }}
                                    className="text-muted-foreground hover:text-foreground opacity-0 transition-opacity group-hover:opacity-100"
                                    title="Open chunk detail"
                                >
                                    <Eye className="size-3.5" />
                                </Link>
                                <button
                                    onClick={() => {
                                        const url = `${window.location.origin}/docs?id=${detail.id}&section=${chunk.id}`;
                                        navigator.clipboard.writeText(url);
                                        onSetCopiedId(chunk.id);
                                        setTimeout(() => onSetCopiedId(null), 1500);
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
                                <button
                                    onClick={() => {
                                        if (editingChunkId === chunk.id) {
                                            onSetEditingChunkId(null);
                                        } else {
                                            onSetEditingChunkId(chunk.id);
                                            onSetEditContent(chunk.content);
                                        }
                                    }}
                                    className={`text-muted-foreground hover:text-foreground opacity-0 transition-opacity group-hover:opacity-100 ${editingChunkId === chunk.id ? "!opacity-100 text-foreground" : ""}`}
                                    title={editingChunkId === chunk.id ? "Cancel editing" : "Edit this section"}
                                >
                                    <Pencil className="size-3.5" />
                                </button>
                            </div>
                            {editingChunkId === chunk.id ? (
                                <EditSection
                                    chunk={chunk}
                                    editContent={editContent}
                                    onSetEditContent={onSetEditContent}
                                    onSetEditingChunkId={onSetEditingChunkId}
                                    saveMutation={saveMutation}
                                />
                            ) : (
                                <div className="prose prose-sm dark:prose-invert max-w-none">
                                    <MarkdownRenderer>{chunk.content}</MarkdownRenderer>
                                </div>
                            )}
                        </section>

                        {/* Add section button */}
                        <AddSectionRow
                            chunk={chunk}
                            idx={idx}
                            addingAfter={addingAfter}
                            newSectionTitle={newSectionTitle}
                            newSectionContent={newSectionContent}
                            addSectionMutation={addSectionMutation}
                            onSetAddingAfter={onSetAddingAfter}
                            onSetNewSectionTitle={onSetNewSectionTitle}
                            onSetNewSectionContent={onSetNewSectionContent}
                        />
                    </Fragment>
                ))}
            </div>

            {/* Prev / next navigation */}
            {(prevDoc || nextDoc) && (
                <div className="border-border mt-10 flex items-center justify-between border-t pt-6">
                    {prevDoc ? (
                        <button
                            onClick={() => onSelectDoc(prevDoc.id)}
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
                            onClick={() => onSelectDoc(nextDoc.id)}
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
    );
}

/* ─── Edit section (inline sub-component) ─── */

interface EditSectionProps {
    chunk: DocumentChunk;
    editContent: string;
    onSetEditContent: (content: string) => void;
    onSetEditingChunkId: (id: string | null) => void;
    saveMutation: UseMutationResult<unknown, Error, { id: string; content: string }>;
}

function EditSection({ chunk, editContent, onSetEditContent, onSetEditingChunkId, saveMutation }: EditSectionProps) {
    return (
        <div className="space-y-3">
            <textarea
                value={editContent}
                onChange={e => onSetEditContent(e.target.value)}
                className="border-input bg-background w-full min-h-[200px] rounded-md border p-3 font-mono text-sm leading-relaxed focus:ring-2 focus:ring-ring outline-none resize-y"
                autoFocus
            />
            <div className="flex items-center gap-2">
                <button
                    onClick={() => saveMutation.mutate({ id: chunk.id, content: editContent })}
                    disabled={saveMutation.isPending || editContent === chunk.content}
                    className="bg-foreground text-background hover:bg-foreground/90 rounded-md px-3 py-1.5 text-sm font-medium disabled:opacity-50"
                >
                    {saveMutation.isPending ? "Saving..." : "Save"}
                </button>
                <button
                    onClick={() => { onSetEditingChunkId(null); onSetEditContent(""); }}
                    className="text-muted-foreground hover:text-foreground text-sm"
                >
                    Cancel
                </button>
            </div>
        </div>
    );
}

/* ─── Add section row (inline sub-component) ─── */

interface AddSectionRowProps {
    chunk: DocumentChunk;
    idx: number;
    addingAfter: number | null;
    newSectionTitle: string;
    newSectionContent: string;
    addSectionMutation: UseMutationResult<unknown, Error, { title: string; content: string; afterOrder: number }>;
    onSetAddingAfter: (order: number | null) => void;
    onSetNewSectionTitle: (title: string) => void;
    onSetNewSectionContent: (content: string) => void;
}

function AddSectionRow({
    chunk,
    idx,
    addingAfter,
    newSectionTitle,
    newSectionContent,
    addSectionMutation,
    onSetAddingAfter,
    onSetNewSectionTitle,
    onSetNewSectionContent,
}: AddSectionRowProps) {
    return (
        <div className="flex justify-center py-0.5">
            {addingAfter === (chunk.documentOrder ?? idx) ? (
                <div className="border-border w-full rounded-lg border p-4 space-y-3">
                    <input
                        type="text"
                        placeholder="Section title"
                        value={newSectionTitle}
                        onChange={e => onSetNewSectionTitle(e.target.value)}
                        className="border-input bg-background w-full rounded-md border px-3 py-2 text-sm"
                        autoFocus
                    />
                    <textarea
                        placeholder="Section content (markdown)"
                        value={newSectionContent}
                        onChange={e => onSetNewSectionContent(e.target.value)}
                        className="border-input bg-background w-full min-h-[100px] rounded-md border p-3 font-mono text-sm resize-y"
                    />
                    <div className="flex gap-2">
                        <button
                            onClick={() => addSectionMutation.mutate({
                                title: newSectionTitle,
                                content: newSectionContent,
                                afterOrder: chunk.documentOrder ?? idx
                            })}
                            disabled={!newSectionTitle.trim() || addSectionMutation.isPending}
                            className="bg-foreground text-background rounded-md px-3 py-1.5 text-sm font-medium disabled:opacity-50"
                        >
                            {addSectionMutation.isPending ? "Adding..." : "Add Section"}
                        </button>
                        <button
                            onClick={() => { onSetAddingAfter(null); onSetNewSectionTitle(""); onSetNewSectionContent(""); }}
                            className="text-muted-foreground hover:text-foreground text-sm"
                        >
                            Cancel
                        </button>
                    </div>
                </div>
            ) : (
                <button
                    onClick={() => onSetAddingAfter(chunk.documentOrder ?? idx)}
                    className="text-muted-foreground/0 hover:text-muted-foreground group flex items-center gap-1 w-full transition-colors"
                >
                    <div className="bg-border/0 group-hover:bg-border h-px flex-1 transition-colors" />
                    <Plus className="size-4" />
                    <div className="bg-border/0 group-hover:bg-border h-px flex-1 transition-colors" />
                </button>
            )}
        </div>
    );
}
