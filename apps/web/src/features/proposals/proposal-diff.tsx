interface ProposalDiffProps {
    currentChunk: { title: string; content: string; type: string; rationale?: string | null; [key: string]: unknown };
    changes: Record<string, unknown>;
}

const FIELD_LABELS: Record<string, string> = {
    title: "Title",
    content: "Content",
    type: "Type",
    tags: "Tags",
    rationale: "Rationale",
    alternatives: "Alternatives",
    consequences: "Consequences",
    scope: "Scope"
};

export function ProposalDiff({ currentChunk, changes }: ProposalDiffProps) {
    const changedFields = Object.keys(changes).filter(k => k in FIELD_LABELS);

    if (changedFields.length === 0) {
        return <div className="text-muted-foreground text-xs">No changes</div>;
    }

    return (
        <div className="space-y-3">
            {changedFields.map(field => (
                <div key={field} className="space-y-1">
                    <div className="text-muted-foreground text-[10px] font-semibold tracking-wider uppercase">
                        {FIELD_LABELS[field] ?? field}
                    </div>
                    <FieldDiff field={field} current={currentChunk[field]} proposed={changes[field]} />
                </div>
            ))}
        </div>
    );
}

function FieldDiff({ field, current, proposed }: { field: string; current: unknown; proposed: unknown }) {
    if (field === "tags") {
        const currentTags = (current as string[] | undefined) ?? [];
        const proposedTags = (proposed as string[]) ?? [];
        const added = proposedTags.filter(t => !currentTags.includes(t));
        const removed = currentTags.filter(t => !proposedTags.includes(t));
        const kept = currentTags.filter(t => proposedTags.includes(t));
        return (
            <div className="flex flex-wrap gap-1 text-xs">
                {kept.map(t => (
                    <span key={t} className="bg-muted rounded px-1.5 py-0.5">
                        {t}
                    </span>
                ))}
                {added.map(t => (
                    <span key={`+${t}`} className="rounded border border-emerald-500/30 bg-emerald-500/15 px-1.5 py-0.5 text-emerald-500">
                        + {t}
                    </span>
                ))}
                {removed.map(t => (
                    <span key={`-${t}`} className="rounded border border-red-500/30 bg-red-500/15 px-1.5 py-0.5 text-red-500 line-through">
                        − {t}
                    </span>
                ))}
            </div>
        );
    }

    if (field === "alternatives") {
        const currentAlts = (current as string[] | undefined) ?? [];
        const proposedAlts = (proposed as string[]) ?? [];
        return (
            <div className="text-xs">
                <div className="text-red-500/80 line-through">{currentAlts.join(", ") || "(none)"}</div>
                <div className="text-emerald-500/80">{proposedAlts.join(", ") || "(none)"}</div>
            </div>
        );
    }

    if (field === "scope") {
        return (
            <div className="font-mono text-xs">
                <div className="text-red-500/80 line-through">{JSON.stringify(current ?? {})}</div>
                <div className="text-emerald-500/80">{JSON.stringify(proposed ?? {})}</div>
            </div>
        );
    }

    // Default: text diff for title, content, type, rationale, consequences
    const currentStr = String(current ?? "");
    const proposedStr = String(proposed ?? "");

    if (field === "content" && currentStr.length > 300) {
        return (
            <div className="space-y-1 text-xs">
                <details>
                    <summary className="text-muted-foreground hover:text-foreground cursor-pointer">
                        Content changed ({currentStr.length} → {proposedStr.length} chars)
                    </summary>
                    <div className="mt-1 space-y-1 rounded border p-2">
                        <div className="whitespace-pre-wrap text-red-500/80 line-through">{currentStr}</div>
                        <div className="whitespace-pre-wrap text-emerald-500/80">{proposedStr}</div>
                    </div>
                </details>
            </div>
        );
    }

    return (
        <div className="space-y-0.5 text-xs">
            <div className="whitespace-pre-wrap text-red-500/80 line-through">{currentStr || "(empty)"}</div>
            <div className="whitespace-pre-wrap text-emerald-500/80">{proposedStr || "(empty)"}</div>
        </div>
    );
}
