import { Code, FileCode } from "lucide-react";

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
}

export function MoreContextContextTab({ chunkId, appliesTo, fileReferences }: MoreContextContextTabProps) {
    return (
        <div className="space-y-6 px-1 pb-4">
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
