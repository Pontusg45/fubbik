import { Code, FileCode, Scale } from "lucide-react";

import { MarkdownRenderer } from "@/components/markdown-renderer";
import { AiSection } from "@/features/chunks/ai-section";

export interface AppliesTo {
    id: string;
    pattern: string;
    note?: string | null;
}

export interface FileReference {
    id: string;
    path: string;
    anchor?: string | null;
    relation: string;
}

export interface MoreContextContextTabProps {
    chunkId: string;
    appliesTo?: AppliesTo[];
    fileReferences?: FileReference[];
    rationale?: string | null;
    alternatives?: string[] | null;
    consequences?: string | null;
}

export function MoreContextContextTab({ chunkId, appliesTo, fileReferences, rationale, alternatives, consequences }: MoreContextContextTabProps) {
    return (
        <div className="space-y-6 px-1 pb-4">
            {(rationale || (alternatives && alternatives.length > 0) || consequences) && (
                <section>
                    <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-amber-600 dark:text-amber-400">
                        <Scale className="size-3.5" />
                        Decision context
                    </h3>
                    <div className="rounded-md border-l-2 border-amber-500/40 bg-amber-500/5 px-4 py-3 space-y-3">
                        {rationale && (
                            <div>
                                <div className="mb-1 text-xs font-semibold text-muted-foreground">Rationale</div>
                                <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground">
                                    <MarkdownRenderer>{rationale}</MarkdownRenderer>
                                </div>
                            </div>
                        )}
                        {alternatives && alternatives.length > 0 && (
                            <div>
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
                    </div>
                </section>
            )}
            <section>
                <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    <Code className="size-3.5" />
                    Applies to
                </h3>
                {appliesTo && appliesTo.length > 0 ? (
                    <div className="space-y-1">
                        {appliesTo.map(applies => (
                            <div key={applies.id} className="rounded border px-3 py-2 text-sm">
                                <code className="font-mono text-xs">{applies.pattern}</code>
                                {applies.note && (
                                    <p className="mt-1 text-xs text-muted-foreground">{applies.note}</p>
                                )}
                            </div>
                        ))}
                    </div>
                ) : (
                    <p className="text-xs text-muted-foreground">No file patterns associated.</p>
                )}
            </section>

            <section>
                <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    <FileCode className="size-3.5" />
                    File references
                </h3>
                {fileReferences && fileReferences.length > 0 ? (
                    <div className="space-y-1">
                        {fileReferences.map(ref => (
                            <div key={ref.id} className="rounded border px-3 py-2 text-sm">
                                <code className="font-mono text-xs">{ref.path}</code>
                                {ref.anchor && (
                                    <span className="ml-2 text-xs text-muted-foreground">@ {ref.anchor}</span>
                                )}
                                <span className="ml-2 text-xs text-muted-foreground">({ref.relation})</span>
                            </div>
                        ))}
                    </div>
                ) : (
                    <p className="text-xs text-muted-foreground">No file references.</p>
                )}
            </section>

            <section>
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    AI enrichment
                </h3>
                <AiSection chunkId={chunkId} />
            </section>
        </div>
    );
}
