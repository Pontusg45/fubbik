import { useQuery } from "@tanstack/react-query";
import {
    BookOpen,
    CheckSquare,
    Compass,
    Database,
    FileText,
    Lightbulb,
    Scale,
    StickyNote,
    Wrench,
    type LucideIcon
} from "lucide-react";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

/**
 * Thin client hooks over /api/chunk-types and /api/connection-relations.
 *
 * Vocabularies change rarely — cache aggressively. Consumers shouldn't have
 * to think about loading states for most rendering paths; either use the data
 * when it arrives, or fall back to a sensible default the first render.
 */

const STALE_FOREVER = 1000 * 60 * 60 * 24; // 24h — close enough to "forever" for a React process

export interface ChunkTypeMeta {
    id: string;
    label: string;
    description: string | null;
    icon: string | null;
    color: string;
    examples: string[];
    displayOrder: number;
    builtIn: boolean;
}

export interface ConnectionRelationMeta {
    id: string;
    label: string;
    description: string | null;
    arrowStyle: "solid" | "dashed" | "dotted";
    direction: "forward" | "bidirectional";
    color: string;
    inverseOfId: string | null;
    displayOrder: number;
    builtIn: boolean;
}

export function useChunkTypes(codebaseId?: string) {
    return useQuery({
        queryKey: ["chunk-types", codebaseId ?? null],
        queryFn: async () =>
            unwrapEden(await api.api["chunk-types"].get({ query: codebaseId ? { codebaseId } : ({} as never) })) as ChunkTypeMeta[],
        staleTime: STALE_FOREVER,
        gcTime: STALE_FOREVER
    });
}

export function useConnectionRelations(codebaseId?: string) {
    return useQuery({
        queryKey: ["connection-relations", codebaseId ?? null],
        queryFn: async () =>
            unwrapEden(
                await api.api["connection-relations"].get({ query: codebaseId ? { codebaseId } : ({} as never) })
            ) as ConnectionRelationMeta[],
        staleTime: STALE_FOREVER,
        gcTime: STALE_FOREVER
    });
}

/**
 * Registry of Lucide icons that can be referenced by name from the chunk_type.icon column.
 * When adding a new icon to the DB, add it here too — unknown icon names fall back to FileText.
 * Keeping this an explicit list (rather than doing dynamic lookup on the whole Lucide catalog)
 * keeps the bundle small and gives us a stable contract for what's renderable.
 */
export const CHUNK_TYPE_ICONS: Record<string, LucideIcon> = {
    StickyNote,
    FileText,
    BookOpen,
    Compass,
    Database,
    CheckSquare,
    Scale,
    Lightbulb,
    Wrench
};

export function resolveChunkTypeIcon(iconName: string | null | undefined): LucideIcon {
    if (!iconName) return FileText;
    return CHUNK_TYPE_ICONS[iconName] ?? FileText;
}

/**
 * Returns metadata for a single chunk type, merging the DB catalog with a sync default
 * so the first render never shows nothing. Prefer this over prop-drilling metadata.
 */
export function useChunkTypeMeta(typeSlug: string | null | undefined, codebaseId?: string): ChunkTypeMeta {
    const { data } = useChunkTypes(codebaseId);
    const slug = typeSlug ?? "note";
    const match = data?.find(t => t.id === slug);
    if (match) return match;
    return {
        id: slug,
        label: slug.charAt(0).toUpperCase() + slug.slice(1),
        description: null,
        icon: null,
        color: "#8b5cf6",
        examples: [],
        displayOrder: 999,
        builtIn: false
    };
}

export function useRelationMeta(relationSlug: string | null | undefined, codebaseId?: string): ConnectionRelationMeta {
    const { data } = useConnectionRelations(codebaseId);
    const slug = relationSlug ?? "related_to";
    const match = data?.find(r => r.id === slug);
    if (match) return match;
    return {
        id: slug,
        label: slug.replace(/_/g, " "),
        description: null,
        arrowStyle: "solid",
        direction: "forward",
        color: "#6b7280",
        inverseOfId: null,
        displayOrder: 999,
        builtIn: false
    };
}

export function useRelationColor(relationSlug: string | null | undefined, codebaseId?: string): string {
    return useRelationMeta(relationSlug, codebaseId).color;
}

/**
 * Given a forward-direction relation slug, returns the catalog row for its inverse
 * (if one is registered via inverse_of_id). Used on chunk detail to render incoming
 * edges with the inverse label — e.g., "depends_on" shown as "required_by" on the
 * target chunk's page. Returns null for bidirectional or unlinked relations.
 */
export function useInverseRelationMeta(
    relationSlug: string | null | undefined,
    codebaseId?: string
): ConnectionRelationMeta | null {
    const { data } = useConnectionRelations(codebaseId);
    if (!relationSlug || !data) return null;
    const forward = data.find(r => r.id === relationSlug);
    if (!forward?.inverseOfId) return null;
    return data.find(r => r.id === forward.inverseOfId) ?? null;
}

/**
 * Mounts at app root. Renders nothing — just kicks off the catalog queries so the
 * react-query cache has `chunk-types` / `connection-relations` populated before
 * sync callers like `relationColor()` need them.
 */
export function VocabularyPrimer() {
    useChunkTypes();
    useConnectionRelations();
    return null;
}
