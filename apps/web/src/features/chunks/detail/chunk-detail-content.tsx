import { Bot, Clock, Scale, Star } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { MarkdownRenderer } from "@/components/markdown-renderer";
import { ChunkLinkRenderer } from "@/features/chunks/chunk-link-renderer";
import { ChunkTypeIcon } from "@/features/chunks/chunk-type-icon";
import { estimateReadingTime } from "@/features/chunks/reading-time";
import { StalenessBanner } from "@/features/staleness/staleness-banner";

export interface ChunkDetailContentProps {
    chunkId: string;
    type: string;
    title: string;
    content: string;
    summary?: string | null;
    updatedAt: string | Date;
    isAi?: boolean;
    reviewStatus?: string;
    isFavorite?: boolean;
    onToggleFavorite?: () => void;
    rationale?: string | null;
    alternatives?: string[] | null;
    consequences?: string | null;
    readerClasses: string;
}

function relativeDate(date: Date): string {
    const diff = Date.now() - date.getTime();
    const days = Math.floor(diff / (1000 * 60 * 60 * 24));
    if (days === 0) return "today";
    if (days === 1) return "yesterday";
    if (days < 7) return `${days} days ago`;
    if (days < 30) return `${Math.floor(days / 7)}w ago`;
    if (days < 365) return `${Math.floor(days / 30)}mo ago`;
    return `${Math.floor(days / 365)}y ago`;
}

export function ChunkDetailContent({
    chunkId,
    type,
    title,
    content,
    summary,
    updatedAt,
    isAi,
    reviewStatus,
    isFavorite,
    onToggleFavorite,
    rationale,
    alternatives,
    consequences,
    readerClasses,
}: ChunkDetailContentProps) {
    const updated = new Date(updatedAt);
    const reading = estimateReadingTime(content);
    const hasDecisionContext = !!rationale || (alternatives && alternatives.length > 0) || !!consequences;

    return (
        <div className="flex-1 min-w-0 max-w-[760px] mx-auto" data-focus-main="true">
            {/* Meta row */}
            <div className="mb-4 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
                <span className="inline-flex items-center gap-1.5">
                    <ChunkTypeIcon type={type} className="size-3.5" />
                    <span className="font-mono">{type}</span>
                </span>
                <span>·</span>
                <span className="inline-flex items-center gap-1">
                    <Clock className="size-3" />
                    {reading.label}
                </span>
                <span>·</span>
                <span>Updated {relativeDate(updated)}</span>
                {isAi && (
                    <>
                        <span>·</span>
                        <Badge
                            variant="outline"
                            size="sm"
                            className={
                                reviewStatus === "draft"
                                    ? "border-yellow-500/30 bg-yellow-500/10 text-yellow-600"
                                    : reviewStatus === "reviewed"
                                      ? "border-blue-500/30 bg-blue-500/10 text-blue-600"
                                      : "border-green-500/30 bg-green-500/10 text-green-600"
                            }
                        >
                            <Bot className="mr-1 size-3" />
                            AI {reviewStatus === "draft" ? "Draft" : reviewStatus === "reviewed" ? "Reviewed" : "Approved"}
                        </Badge>
                    </>
                )}
            </div>

            {/* Title with favorite */}
            <div className="mb-3 flex items-start gap-3">
                <h1 className="text-3xl font-bold tracking-tight leading-tight flex-1">{title}</h1>
                {onToggleFavorite && (
                    <button
                        type="button"
                        onClick={onToggleFavorite}
                        className="mt-1.5 text-muted-foreground hover:text-yellow-500 transition-colors"
                        title={isFavorite ? "Remove from favorites" : "Add to favorites"}
                    >
                        <Star className={`size-5 ${isFavorite ? "fill-yellow-500 text-yellow-500" : ""}`} />
                    </button>
                )}
            </div>

            {/* Summary */}
            {summary && (
                <p className="mb-6 text-lg italic text-muted-foreground leading-relaxed">{summary}</p>
            )}

            {/* Staleness banner */}
            <StalenessBanner chunkId={chunkId} />

            {/* Content */}
            <div className={`prose dark:prose-invert max-w-none ${readerClasses}`}>
                <ChunkLinkRenderer content={content} currentChunkId={chunkId} />
            </div>

            {/* Decision context callout */}
            {hasDecisionContext && (
                <aside className="mt-10 rounded-md border-l-2 border-amber-500/40 bg-amber-500/5 px-5 py-4">
                    <div className="mb-3 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-amber-600 dark:text-amber-400">
                        <Scale className="size-3.5" />
                        Decision context
                    </div>
                    {rationale && (
                        <div className="mb-4">
                            <div className="mb-1 text-xs font-semibold text-muted-foreground">Rationale</div>
                            <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground">
                                <MarkdownRenderer>{rationale}</MarkdownRenderer>
                            </div>
                        </div>
                    )}
                    {alternatives && alternatives.length > 0 && (
                        <div className="mb-4">
                            <div className="mb-1 text-xs font-semibold text-muted-foreground">Alternatives considered</div>
                            <ul className="text-sm text-muted-foreground list-disc pl-5 space-y-1">
                                {alternatives.map((alt, i) => (
                                    <li key={i}>{alt}</li>
                                ))}
                            </ul>
                        </div>
                    )}
                    {consequences && (
                        <div>
                            <div className="mb-1 text-xs font-semibold text-muted-foreground">Consequences</div>
                            <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground">
                                <MarkdownRenderer>{consequences}</MarkdownRenderer>
                            </div>
                        </div>
                    )}
                </aside>
            )}
        </div>
    );
}
