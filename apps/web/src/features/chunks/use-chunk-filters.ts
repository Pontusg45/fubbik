import { useNavigate } from "@tanstack/react-router";
import { Route } from "@/routes/chunks.index";

export type ChunkSearchParams = {
    type?: string;
    q?: string;
    sort?: string;
    tags?: string;
    size?: string;
    after?: string;
    enrichment?: string;
    minConnections?: string;
    group?: string;
    collection?: string;
    view?: string;
    origin?: string;
    reviewStatus?: string;
    allCodebases?: string;
};

export function useChunkFilters() {
    const navigate = useNavigate({ from: "/chunks/" });
    const { type, q, sort, tags, size, after, enrichment, minConnections, group, collection, view, origin, reviewStatus, allCodebases } = Route.useSearch();

    const activeTags = tags ? tags.split(",") : [];
    const activeFilterCount = [tags, size, after, enrichment, minConnections, origin, reviewStatus].filter(Boolean).length;
    const hasActiveFilters = !!(type || q || sort || tags || size || after || enrichment || minConnections || origin || reviewStatus);
    const isFederated = allCodebases === "true";

    function updateSearch(params: Partial<ChunkSearchParams>) {
        navigate({
            search: {
                type: params.type !== undefined ? params.type : type,
                q: params.q !== undefined ? params.q : q,
                sort: params.sort !== undefined ? params.sort : sort,
                tags: params.tags !== undefined ? params.tags : tags,
                size: params.size !== undefined ? params.size : size,
                after: params.after !== undefined ? params.after : after,
                enrichment: params.enrichment !== undefined ? params.enrichment : enrichment,
                minConnections: params.minConnections !== undefined ? params.minConnections : minConnections,
                group: params.group !== undefined ? params.group : group,
                collection: params.collection !== undefined ? params.collection : collection,
                view: params.view !== undefined ? params.view : view,
                origin: params.origin !== undefined ? params.origin : origin,
                reviewStatus: params.reviewStatus !== undefined ? params.reviewStatus : reviewStatus,
                allCodebases: params.allCodebases !== undefined ? params.allCodebases : allCodebases
            }
        });
    }

    function clearAllFilters() {
        navigate({
            search: {
                type: undefined,
                q: undefined,
                sort: undefined,
                tags: undefined,
                size: undefined,
                after: undefined,
                enrichment: undefined,
                minConnections: undefined,
                group,
                collection,
                view,
                origin: undefined,
                reviewStatus: undefined,
                allCodebases: undefined
            }
        });
    }

    function toggleTag(tag: string) {
        const next = activeTags.includes(tag) ? activeTags.filter(t => t !== tag) : [...activeTags, tag];
        updateSearch({ tags: next.length > 0 ? next.join(",") : undefined });
    }

    return {
        // Current filter values
        type,
        q,
        sort,
        tags,
        size,
        after,
        enrichment,
        minConnections,
        group,
        collection,
        view,
        origin,
        reviewStatus,
        allCodebases,
        // Derived
        activeTags,
        activeFilterCount,
        hasActiveFilters,
        isFederated,
        // Actions
        updateSearch,
        clearAllFilters,
        toggleTag,
    };
}
