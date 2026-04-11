import { Link } from "@tanstack/react-router";

import { Badge } from "@/components/ui/badge";
import { ChunkHealthBadge } from "@/features/chunks/chunk-health-badge";
import { ChunkToc } from "@/features/chunks/chunk-toc";
import { getChunkSize } from "@/features/chunks/chunk-size";
import { InlineTagEditor } from "@/features/chunks/inline-tag-editor";

export interface ChunkMetadataPanelProps {
    content: string;
    tags: Array<{ id: string; name: string }>;
    onTagsUpdate: (tags: string[]) => void;
    tagsLoading?: boolean;
    type: string;
    createdAt: string | Date;
    updatedAt: string | Date;
    connectionCount: number;
    codebases?: Array<{ id: string; name: string }>;
    origin?: string;
    reviewStatus?: string;
    healthScore?: {
        total: number;
        breakdown: { freshness: number; completeness: number; richness: number; connectivity: number };
        issues: string[];
    };
    onShowConnections: () => void;
}

export function ChunkMetadataPanel({
    content,
    tags,
    onTagsUpdate,
    tagsLoading,
    type,
    createdAt,
    updatedAt,
    connectionCount,
    codebases,
    origin,
    reviewStatus,
    healthScore,
    onShowConnections,
}: ChunkMetadataPanelProps) {
    const size = getChunkSize(content);
    const created = new Date(createdAt);
    const updated = new Date(updatedAt);
    const primaryCodebase = codebases && codebases.length > 0 ? codebases[0] : undefined;

    return (
        <aside
            className="hidden lg:block w-[220px] shrink-0 print:hidden"
            data-focus-hide="true"
        >
            <div className="sticky top-8 space-y-6">
                <ChunkToc content={content} />

                <div className="border-t pt-4">
                    <div className="text-[9px] font-semibold uppercase tracking-wider text-muted-foreground mb-3">
                        Details
                    </div>

                    {healthScore && (
                        <div className="mb-3 flex items-center justify-between">
                            <span className="text-[11px] text-muted-foreground">Health</span>
                            <ChunkHealthBadge healthScore={healthScore} />
                        </div>
                    )}

                    <div className="mb-4">
                        <div className="text-[11px] text-muted-foreground mb-1.5">Tags</div>
                        <InlineTagEditor
                            tags={tags}
                            onUpdate={onTagsUpdate}
                            loading={tagsLoading}
                        />
                    </div>

                    <dl className="space-y-1.5 text-[11px]">
                        <MetaRow label="Type">
                            <Link to="/chunks" search={{ type } as any} className="hover:text-foreground transition-colors">
                                {type}
                            </Link>
                        </MetaRow>
                        <MetaRow label="Created">
                            <span title={created.toLocaleString()}>{created.toLocaleDateString()}</span>
                        </MetaRow>
                        <MetaRow label="Updated">
                            <span title={updated.toLocaleString()}>{updated.toLocaleDateString()}</span>
                        </MetaRow>
                        <MetaRow label="Size">
                            <span style={{ color: size.color }}>{size.lines} ln</span>
                        </MetaRow>
                        <MetaRow label="Connections">
                            <button
                                type="button"
                                onClick={onShowConnections}
                                className="hover:text-foreground transition-colors underline-offset-2 hover:underline"
                            >
                                {connectionCount}
                            </button>
                        </MetaRow>
                        {primaryCodebase && (
                            <MetaRow label="Codebase">
                                <Link
                                    to="/codebases/$codebaseId"
                                    params={{ codebaseId: primaryCodebase.id }}
                                    className="hover:text-foreground transition-colors truncate"
                                >
                                    {primaryCodebase.name}
                                </Link>
                            </MetaRow>
                        )}
                        {origin && (
                            <MetaRow label="Origin">
                                <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                                    {origin}
                                </Badge>
                            </MetaRow>
                        )}
                        {reviewStatus && (
                            <MetaRow label="Review">
                                <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                                    {reviewStatus}
                                </Badge>
                            </MetaRow>
                        )}
                    </dl>
                </div>
            </div>
        </aside>
    );
}

function MetaRow({ label, children }: { label: string; children: React.ReactNode }) {
    return (
        <div className="flex items-center justify-between gap-2">
            <dt className="text-muted-foreground shrink-0">{label}</dt>
            <dd className="text-right min-w-0 truncate">{children}</dd>
        </div>
    );
}
