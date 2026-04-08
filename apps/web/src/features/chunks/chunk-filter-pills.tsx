import { X } from "lucide-react";

const PILL_COLORS: Record<string, string> = {
    type: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    origin: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    reviewStatus: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    tags: "bg-teal-500/15 border-teal-500/30 text-teal-400",
    q: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    after: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    enrichment: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    minConnections: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    allCodebases: "bg-slate-500/15 border-slate-500/30 text-slate-400",
};

function Pill({ colorKey, label, onRemove }: { colorKey: string; label: string; onRemove: () => void }) {
    const colorClass = PILL_COLORS[colorKey] ?? "bg-slate-500/15 border-slate-500/30 text-slate-400";
    return (
        <div className={`flex items-center gap-1 rounded border px-2 py-0.5 text-xs ${colorClass}`}>
            <span>{label}</span>
            <button
                onClick={onRemove}
                className="ml-0.5 rounded opacity-60 transition-opacity hover:opacity-100"
                aria-label={`Remove ${label} filter`}
            >
                <X className="size-3" />
            </button>
        </div>
    );
}

interface ChunkFilterPillsProps {
    type?: string;
    q?: string;
    tags?: string;
    after?: string;
    enrichment?: string;
    minConnections?: string;
    origin?: string;
    reviewStatus?: string;
    allCodebases?: string;
    activeTags: string[];
    onRemoveFilter: (key: string) => void;
    onClearAll: () => void;
}

export function ChunkFilterPills({
    type,
    q,
    after,
    enrichment,
    minConnections,
    origin,
    reviewStatus,
    allCodebases,
    activeTags,
    onRemoveFilter,
    onClearAll,
}: ChunkFilterPillsProps) {
    const hasAny = type || q || activeTags.length > 0 || after || enrichment || minConnections || origin || reviewStatus || allCodebases;
    if (!hasAny) return null;

    return (
        <div className="mb-4 flex flex-wrap items-center gap-1.5">
            {type && (
                <Pill colorKey="type" label={`type: ${type}`} onRemove={() => onRemoveFilter("type")} />
            )}
            {q && (
                <Pill colorKey="q" label={`search: ${q}`} onRemove={() => onRemoveFilter("q")} />
            )}
            {activeTags.map(tag => (
                <Pill key={tag} colorKey="tags" label={tag} onRemove={() => onRemoveFilter(`tag:${tag}`)} />
            ))}
            {after && (
                <Pill colorKey="after" label={`updated: ${after}d`} onRemove={() => onRemoveFilter("after")} />
            )}
            {enrichment && (
                <Pill colorKey="enrichment" label={`enrichment: ${enrichment}`} onRemove={() => onRemoveFilter("enrichment")} />
            )}
            {minConnections && (
                <Pill colorKey="minConnections" label={`connections: ${minConnections}+`} onRemove={() => onRemoveFilter("minConnections")} />
            )}
            {origin && (
                <Pill colorKey="origin" label={`origin: ${origin}`} onRemove={() => onRemoveFilter("origin")} />
            )}
            {reviewStatus && (
                <Pill colorKey="reviewStatus" label={`review: ${reviewStatus}`} onRemove={() => onRemoveFilter("reviewStatus")} />
            )}
            {allCodebases && (
                <Pill colorKey="allCodebases" label="all codebases" onRemove={() => onRemoveFilter("allCodebases")} />
            )}
            <button
                onClick={onClearAll}
                className="text-muted-foreground hover:text-foreground ml-1 text-xs underline"
            >
                Clear all
            </button>
        </div>
    );
}
