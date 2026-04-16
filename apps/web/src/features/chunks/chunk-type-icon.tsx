import { resolveChunkTypeIcon, useChunkTypeMeta } from "@/features/vocabularies/use-vocabularies";

/**
 * Chunk type icon — resolved from the chunk_type catalog (DB-driven).
 * Unknown icon names fall back to FileText via resolveChunkTypeIcon.
 */
export function ChunkTypeIcon({ type, className }: { type: string; className?: string }) {
    const meta = useChunkTypeMeta(type);
    const Icon = resolveChunkTypeIcon(meta.icon);
    return <Icon className={className ?? "size-3.5"} />;
}
