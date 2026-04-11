import { Link, useNavigate } from "@tanstack/react-router";
import {
    Archive,
    ArrowLeft,
    Check,
    Copy,
    Download,
    Edit,
    Eye,
    Flag,
    Focus,
    MoreHorizontal,
    Network,
    Scissors,
    Sparkles,
    Star,
    Trash2,
} from "lucide-react";
import { useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useFavorites } from "@/features/chunks/use-favorites";
import { useFocusMode } from "@/hooks/use-focus-mode";

export interface ChunkDetailTopBarProps {
    chunkId: string;
    title: string;
    content: string;
    type: string;
    isEntryPoint?: boolean;
    isAi?: boolean;
    reviewStatus?: string;
    onArchive: () => void;
    onDelete: () => void;
    onSplit: () => void;
    onToggleEntryPoint: () => void;
    onReview?: (status: "reviewed" | "approved") => void;
    archivePending?: boolean;
    deletePending?: boolean;
}

function buildMarkdown(title: string, type: string, content: string): string {
    return `# ${title}\n\n**Type:** ${type}\n\n${content}`;
}

function downloadMarkdown(title: string, type: string, content: string) {
    const md = buildMarkdown(title, type, content);
    const blob = new Blob([md], { type: "text/markdown" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${title.replace(/[^a-z0-9]/gi, "-").toLowerCase()}.md`;
    a.click();
    URL.revokeObjectURL(url);
}

export function ChunkDetailTopBar({
    chunkId,
    title,
    content,
    type,
    isEntryPoint,
    isAi,
    reviewStatus,
    onArchive,
    onDelete,
    onSplit,
    onToggleEntryPoint,
    onReview,
    archivePending,
    deletePending,
}: ChunkDetailTopBarProps) {
    const navigate = useNavigate();
    const { toggleFavorite, isFavorite } = useFavorites();
    const { enabled: focusMode, toggle: toggleFocus } = useFocusMode();
    const [showDeleteDialog, setShowDeleteDialog] = useState(false);
    const [exportCopied, setExportCopied] = useState(false);

    const favorited = isFavorite(chunkId);

    function handleCopy() {
        void navigator.clipboard.writeText(buildMarkdown(title, type, content));
        setExportCopied(true);
        setTimeout(() => setExportCopied(false), 2000);
    }

    return (
        <div className="mb-6 flex items-center justify-between print:hidden" data-focus-hide="true">
            <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
                <ArrowLeft className="size-4" />
                Back
            </Button>

            <div className="flex items-center gap-1">
                <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => toggleFavorite(chunkId)}
                    className="gap-1.5"
                    title={favorited ? "Remove from favorites" : "Add to favorites"}
                >
                    <Star className={`size-3.5 ${favorited ? "fill-yellow-500 text-yellow-500" : ""}`} />
                </Button>

                <DropdownMenu>
                    <DropdownMenuTrigger
                        render={
                            <Button variant="ghost" size="sm" className="gap-1.5">
                                <Download className="size-3.5" />
                                Export
                            </Button>
                        }
                    />
                    <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={handleCopy}>
                            {exportCopied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
                            {exportCopied ? "Copied" : "Copy as markdown"}
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={() => downloadMarkdown(title, type, content)}>
                            <Download className="size-3.5" />
                            Download .md
                        </DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>

                <Button
                    variant="outline"
                    size="sm"
                    render={<Link to="/chunks/$chunkId/edit" params={{ chunkId }} />}
                    className="gap-1.5"
                >
                    <Edit className="size-3.5" />
                    Edit
                </Button>

                <DropdownMenu>
                    <DropdownMenuTrigger
                        render={
                            <Button variant="ghost" size="sm" aria-label="More actions">
                                <MoreHorizontal className="size-4" />
                            </Button>
                        }
                    />
                    <DropdownMenuContent align="end" className="w-56">
                        <DropdownMenuItem onClick={toggleFocus}>
                            <Focus className="size-3.5" />
                            {focusMode ? "Exit focus mode" : "Focus mode"}
                            <span className="ml-auto text-[10px] text-muted-foreground font-mono">f</span>
                        </DropdownMenuItem>
                        <DropdownMenuItem
                            onClick={() => void navigate({ to: "/graph", search: { pathFrom: chunkId } as any })}
                        >
                            <Network className="size-3.5" />
                            Find path in graph
                        </DropdownMenuItem>
                        <DropdownMenuItem
                            onClick={() =>
                                void navigate({
                                    to: "/search",
                                    search: { q: `similar-to:"${title}"` } as any,
                                })
                            }
                        >
                            <Sparkles className="size-3.5" />
                            Show similar chunks
                        </DropdownMenuItem>

                        <DropdownMenuSeparator />

                        <DropdownMenuItem onClick={onSplit}>
                            <Scissors className="size-3.5" />
                            Split chunk
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={onToggleEntryPoint}>
                            <Flag className={`size-3.5 ${isEntryPoint ? "fill-emerald-500 text-emerald-500" : ""}`} />
                            {isEntryPoint ? "Unmark entry point" : "Mark as entry point"}
                        </DropdownMenuItem>

                        {isAi && reviewStatus !== "approved" && onReview && (
                            <>
                                <DropdownMenuSeparator />
                                {reviewStatus === "draft" && (
                                    <DropdownMenuItem onClick={() => onReview("reviewed")}>
                                        <Eye className="size-3.5" />
                                        Mark as reviewed
                                    </DropdownMenuItem>
                                )}
                                <DropdownMenuItem onClick={() => onReview("approved")}>
                                    <Check className="size-3.5" />
                                    Approve
                                </DropdownMenuItem>
                            </>
                        )}

                        <DropdownMenuSeparator />

                        <DropdownMenuItem onClick={onArchive} disabled={archivePending}>
                            <Archive className="size-3.5" />
                            {archivePending ? "Archiving…" : "Archive"}
                        </DropdownMenuItem>
                        <DropdownMenuItem
                            onClick={() => setShowDeleteDialog(true)}
                            disabled={deletePending}
                            className="text-destructive focus:text-destructive"
                        >
                            <Trash2 className="size-3.5" />
                            Delete
                        </DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>
            </div>

            <ConfirmDialog
                open={showDeleteDialog}
                onOpenChange={setShowDeleteDialog}
                title="Delete chunk"
                description="Permanently delete this chunk? This cannot be undone."
                confirmLabel="Delete"
                confirmVariant="destructive"
                onConfirm={() => {
                    setShowDeleteDialog(false);
                    onDelete();
                }}
                loading={deletePending}
            />
        </div>
    );
}
