import { useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Bot, Clock, FileText, Pin, Server } from "lucide-react";
import { memo } from "react";

import { Badge } from "@/components/ui/badge";
import { CardPanel } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipProvider, TooltipTrigger, TooltipPopup } from "@/components/ui/tooltip";
import { ChunkRowActions } from "@/features/chunks/chunk-row-actions";
import { getChunkSize } from "@/features/chunks/chunk-size";

// ---------------------------------------------------------------------------
// ChunkPreviewPopup — shown on title hover
// ---------------------------------------------------------------------------

export function ChunkPreviewPopup({
    chunk,
    queryClient
}: {
    chunk: { id: string; content?: string | null; type: string; summary?: string | null };
    queryClient: ReturnType<typeof useQueryClient>;
}) {
    const cached = queryClient.getQueryData<{ tags?: { name: string }[]; summary?: string | null }>(["chunk", chunk.id]);
    const summary = cached?.summary ?? chunk.summary;
    const tags = cached?.tags?.slice(0, 3);
    const content = chunk.content;

    if (!content && !summary) return null;

    return (
        <TooltipPopup side="bottom" align="start" className="max-w-[300px] p-2.5">
            {summary && <p className="text-foreground text-xs font-medium">{summary}</p>}
            {content && (
                <p className={`text-muted-foreground text-xs ${summary ? "mt-1.5" : ""}`}>
                    {content.slice(0, 150)}
                    {content.length > 150 ? "..." : ""}
                </p>
            )}
            {tags && tags.length > 0 && (
                <div className="mt-2 flex flex-wrap gap-1">
                    {tags.map(tag => (
                        <span key={tag.name} className="bg-muted text-muted-foreground rounded px-1.5 py-0.5 text-[10px]">
                            {tag.name}
                        </span>
                    ))}
                </div>
            )}
        </TooltipPopup>
    );
}

// ---------------------------------------------------------------------------
// ChunkRow
// ---------------------------------------------------------------------------

export interface ChunkRowChunk {
    id: string;
    title: string;
    type: string;
    content?: string | null;
    summary?: string | null;
    updatedAt: string | Date;
    reviewStatus?: string | null;
    origin?: string | null;
    codebaseName?: string | null;
}

export interface ChunkRowProps {
    chunk: ChunkRowChunk;
    /** Position in allChunkIds used for shift-click range selection */
    index: number;
    allChunkIds: string[];
    isSelected: boolean;
    isPinned: boolean;
    /** When true, renders the keyboard-navigation highlight ring */
    isKeyboardSelected?: boolean;
    /** When true, show codebase / AI / review-status badges */
    showExtendedBadges?: boolean;
    /** When true, show the federated codebase badge */
    isFederated?: boolean;
    /** Whether this is the last item (no separator after it) */
    isLast?: boolean;
    /** Whether a separator should be rendered above this row */
    showSeparator?: boolean;

    // Inline title editing
    editingChunkId: string | null;
    editTitle: string;
    onStartEditing: (id: string, title: string) => void;
    onCommitEdit: () => void;
    onCancelEdit: () => void;
    onEditTitleChange: (title: string) => void;

    // Actions
    onHover: (id: string) => void;
    onTogglePin: (id: string) => void;
    onSelectionClick: (id: string, index: number, allIds: string[], e: React.MouseEvent) => void;
    onDelete: (id: string, title: string) => void;
    onReviewCycle?: (id: string, currentStatus: string) => void;
}

export const ChunkRow = memo(function ChunkRow({
    chunk,
    index,
    allChunkIds,
    isSelected,
    isPinned,
    isKeyboardSelected = false,
    showExtendedBadges = false,
    isFederated = false,
    showSeparator = false,
    editingChunkId,
    editTitle,
    onStartEditing,
    onCommitEdit,
    onCancelEdit,
    onEditTitleChange,
    onHover,
    onTogglePin,
    onSelectionClick,
    onDelete,
    onReviewCycle
}: ChunkRowProps) {
    const queryClient = useQueryClient();
    const chunkSize = getChunkSize(chunk.content ?? "");

    return (
        <div>
            {showSeparator && <Separator />}
            <CardPanel
                className={`flex items-center gap-3 p-4 transition-colors ${
                    isKeyboardSelected ? "bg-muted/50 ring-primary/50 ring-2 ring-inset" : "hover:bg-muted/50"
                }`}
                onMouseEnter={() => onHover(chunk.id)}
            >
                <Checkbox
                    checked={isSelected}
                    onCheckedChange={() => {
                        /* handled in onClick */
                    }}
                    onClick={(e: React.MouseEvent) => {
                        e.stopPropagation();
                        onSelectionClick(chunk.id, index, allChunkIds, e);
                    }}
                />
                <button
                    onClick={e => {
                        e.preventDefault();
                        e.stopPropagation();
                        onTogglePin(chunk.id);
                    }}
                    className="text-muted-foreground hover:text-foreground"
                >
                    <Pin className={`size-3 ${isPinned ? "fill-current" : ""}`} />
                </button>
                <Link to="/chunks/$chunkId" params={{ chunkId: chunk.id }} className="flex flex-1 items-center justify-between gap-4">
                    <div className="min-w-0">
                        {editingChunkId === chunk.id ? (
                            <input
                                autoFocus
                                className="bg-background focus:ring-ring w-full rounded border px-1 py-0.5 text-sm font-medium focus:ring-2 focus:outline-none"
                                value={editTitle}
                                onChange={e => onEditTitleChange(e.target.value)}
                                onKeyDown={e => {
                                    if (e.key === "Enter") {
                                        e.preventDefault();
                                        onCommitEdit();
                                    }
                                    if (e.key === "Escape") {
                                        e.preventDefault();
                                        onCancelEdit();
                                    }
                                }}
                                onBlur={onCommitEdit}
                                onClick={e => {
                                    e.preventDefault();
                                    e.stopPropagation();
                                }}
                            />
                        ) : (
                            <TooltipProvider delay={300}>
                                <Tooltip>
                                    <TooltipTrigger
                                        render={<p className="truncate text-sm font-medium" />}
                                        onDoubleClick={e => {
                                            e.preventDefault();
                                            e.stopPropagation();
                                            onStartEditing(chunk.id, chunk.title);
                                        }}
                                    >
                                        {chunk.title}
                                    </TooltipTrigger>
                                    <ChunkPreviewPopup chunk={chunk} queryClient={queryClient} />
                                </Tooltip>
                            </TooltipProvider>
                        )}
                        <div className="mt-1 flex items-center gap-2">
                            <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                {chunk.type}
                            </Badge>
                            {showExtendedBadges && isFederated && !!chunk.codebaseName && (
                                <Badge variant="outline" size="sm" className="border-blue-500/30 bg-blue-500/10 text-[10px] text-blue-600">
                                    <Server className="mr-0.5 size-2.5" />
                                    {String(chunk.codebaseName)}
                                </Badge>
                            )}
                            {showExtendedBadges && chunk.origin === "ai" && (
                                <>
                                    <Badge
                                        variant="outline"
                                        size="sm"
                                        className={
                                            chunk.reviewStatus === "draft"
                                                ? "border-yellow-500/30 bg-yellow-500/10 text-[10px] text-yellow-600"
                                                : chunk.reviewStatus === "reviewed"
                                                  ? "border-blue-500/30 bg-blue-500/10 text-[10px] text-blue-600"
                                                  : "border-green-500/30 bg-green-500/10 text-[10px] text-green-600"
                                        }
                                    >
                                        <Bot className="mr-0.5 size-2.5" />
                                        AI
                                    </Badge>
                                    <button
                                        onClick={e => {
                                            e.preventDefault();
                                            e.stopPropagation();
                                            const next =
                                                { draft: "reviewed", reviewed: "approved", approved: "draft" }[
                                                    chunk.reviewStatus ?? "draft"
                                                ] ?? "reviewed";
                                            onReviewCycle?.(chunk.id, next);
                                        }}
                                        className="size-2.5 shrink-0 rounded-full"
                                        style={{
                                            backgroundColor:
                                                chunk.reviewStatus === "approved"
                                                    ? "#22c55e"
                                                    : chunk.reviewStatus === "reviewed"
                                                      ? "#3b82f6"
                                                      : "#f59e0b"
                                        }}
                                        title={`Review: ${chunk.reviewStatus ?? "draft"} (click to change)`}
                                    />
                                </>
                            )}
                        </div>
                    </div>
                    <div className="flex shrink-0 items-center gap-3">
                        {chunkSize.level !== "good" && (
                            <span className="flex items-center gap-1 text-xs" style={{ color: chunkSize.color }}>
                                <FileText className="size-3" />
                                {chunkSize.lines}L
                            </span>
                        )}
                        <span className="text-muted-foreground flex items-center gap-1 text-xs">
                            <Clock className="size-3" />
                            {new Date(chunk.updatedAt).toLocaleDateString()}
                        </span>
                    </div>
                </Link>
                <div onClick={e => e.stopPropagation()}>
                    <ChunkRowActions
                        chunkId={chunk.id}
                        isPinned={isPinned}
                        onTogglePin={() => onTogglePin(chunk.id)}
                        onDelete={() => onDelete(chunk.id, chunk.title)}
                    />
                </div>
            </CardPanel>
        </div>
    );
});
