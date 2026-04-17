import {
    resolveChunkTypeIcon,
    useChunkTypes,
    useConnectionRelations
} from "@/features/vocabularies/use-vocabularies";

/**
 * Top-center legend for the graph.
 *
 * Data sources:
 * - Chunk type + connection relation metadata comes from the DB catalog
 *   (label, color, icon, arrow style, description) — no hardcoded tables.
 * - Counts come from the currently-visible graph data (typeCounts/relationCounts).
 *
 * Interactions:
 * - Clicking a type pill toggles the URL-driven prefilter types.
 * - Clicking a relation pill toggles the in-session relation filter.
 * - Tooltip shows the description from the catalog.
 * - Line-style swatch matches the relation's arrow_style.
 */
interface GraphLegendProps {
    /** Types present in the visible graph data (value = count of chunks). */
    typeCounts: Map<string, number>;
    /** Active prefilter type set (empty = all). */
    activeTypePrefilter: Set<string>;
    /** Toggles a type in the prefilter (commits to URL). */
    onToggleTypePrefilter: (typeId: string) => void;

    /** Relations present in the visible graph data (value = count of edges). */
    relationCounts: Map<string, number>;
    /** In-session relation filter (empty = all). */
    activeRelationFilter: Set<string>;
    /** Toggles a relation in the in-session filter. */
    onToggleRelationFilter: (relation: string) => void;
}

export function GraphLegend({
    typeCounts,
    activeTypePrefilter,
    onToggleTypePrefilter,
    relationCounts,
    activeRelationFilter,
    onToggleRelationFilter
}: GraphLegendProps) {
    const { data: chunkTypes } = useChunkTypes();
    const { data: relations } = useConnectionRelations();

    // Types: only render for those present in the data, ordered by catalog displayOrder.
    const typesInData = (chunkTypes ?? [])
        .filter(t => (typeCounts.get(t.id) ?? 0) > 0)
        .sort((a, b) => a.displayOrder - b.displayOrder);

    // Same for relations.
    const relationsInData = (relations ?? [])
        .filter(r => (relationCounts.get(r.id) ?? 0) > 0)
        .sort((a, b) => a.displayOrder - b.displayOrder);

    if (typesInData.length === 0 && relationsInData.length === 0) return null;

    return (
        <div className="bg-background/85 absolute top-4 left-1/2 z-10 flex max-w-[min(920px,calc(100vw-280px))] -translate-x-1/2 items-center gap-x-3 gap-y-1.5 overflow-x-auto rounded-lg border px-3 py-1.5 backdrop-blur-sm max-md:hidden">
            {typesInData.length > 0 && (
                <div className="flex items-center gap-1.5">
                    <span className="text-muted-foreground text-[9px] font-medium uppercase tracking-wider mr-0.5 shrink-0">Types</span>
                    {typesInData.map(t => {
                        const count = typeCounts.get(t.id) ?? 0;
                        const active = activeTypePrefilter.has(t.id);
                        // When no filter is set, all types are "shown". We display the
                        // pill in a neutral state (not "active") so the user can see
                        // the default.
                        const hasAnyFilter = activeTypePrefilter.size > 0;
                        const Icon = resolveChunkTypeIcon(t.icon);
                        const tooltipParts = [t.description];
                        if (t.examples.length > 0) tooltipParts.push(`Examples: ${t.examples.join(", ")}`);
                        return (
                            <button
                                key={t.id}
                                onClick={() => onToggleTypePrefilter(t.id)}
                                title={tooltipParts.filter(Boolean).join("\n")}
                                aria-pressed={active}
                                className={`group flex shrink-0 items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] transition-colors ${
                                    active
                                        ? "border-primary bg-primary/10 text-primary"
                                        : hasAnyFilter
                                            ? "border-muted text-muted-foreground/60 hover:text-foreground"
                                            : "text-muted-foreground hover:text-foreground"
                                }`}
                            >
                                <Icon className="size-2.5" style={{ color: t.color }} />
                                <span className="truncate max-w-[120px]">{t.label}</span>
                                <span className="text-muted-foreground/70 font-mono text-[9px] tabular-nums">
                                    {count}
                                </span>
                            </button>
                        );
                    })}
                </div>
            )}

            {typesInData.length > 0 && relationsInData.length > 0 && (
                <div className="bg-border h-4 w-px shrink-0" />
            )}

            {relationsInData.length > 0 && (
                <div className="flex items-center gap-1.5">
                    <span className="text-muted-foreground text-[9px] font-medium uppercase tracking-wider mr-0.5 shrink-0">Relations</span>
                    {relationsInData.map(r => {
                        const count = relationCounts.get(r.id) ?? 0;
                        // Active logic for relations: when filter is empty, all shown.
                        // When non-empty, only those in filter are "shown". Legend
                        // visually distinguishes the filtered-out ones.
                        const hasFilter = activeRelationFilter.size > 0;
                        const inFilter = activeRelationFilter.has(r.id);
                        const filteredOut = hasFilter && !inFilter;
                        return (
                            <button
                                key={r.id}
                                onClick={() => onToggleRelationFilter(r.id)}
                                title={r.description ?? undefined}
                                aria-pressed={hasFilter ? inFilter : false}
                                className={`group flex shrink-0 items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] transition-colors ${
                                    inFilter
                                        ? "border-primary bg-primary/10 text-primary"
                                        : filteredOut
                                            ? "border-muted text-muted-foreground/50 opacity-60 hover:opacity-100"
                                            : "text-muted-foreground hover:text-foreground"
                                }`}
                            >
                                <svg width="14" height="6" viewBox="0 0 14 6" className="shrink-0">
                                    <line
                                        x1="0"
                                        y1="3"
                                        x2="12"
                                        y2="3"
                                        stroke={r.color}
                                        strokeWidth="1.5"
                                        strokeDasharray={
                                            r.arrowStyle === "dashed" ? "3,2"
                                                : r.arrowStyle === "dotted" ? "1,2"
                                                : undefined
                                        }
                                    />
                                    {r.direction === "forward" && (
                                        <polygon points="14,3 11,1.5 11,4.5" fill={r.color} />
                                    )}
                                </svg>
                                <span className="truncate max-w-[120px]">{r.label}</span>
                                <span className="text-muted-foreground/70 font-mono text-[9px] tabular-nums">
                                    {count}
                                </span>
                            </button>
                        );
                    })}
                </div>
            )}
        </div>
    );
}
