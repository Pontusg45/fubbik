interface Delta {
    featureId: string;
    delta: Record<string, unknown>;
    priority: number;
}

interface DeltaWithChunk extends Delta {
    chunkId: string;
}

export interface ResolvedMeta {
    _appliedFeatures: string[];
    _hasDeltas: boolean;
}

export function resolveChunk<T extends Record<string, unknown>>(chunk: T, deltas: Delta[]): T & ResolvedMeta {
    if (deltas.length === 0) {
        return { ...chunk, _appliedFeatures: [], _hasDeltas: false };
    }

    const sorted = [...deltas].sort((a, b) => a.priority - b.priority);
    const resolved = { ...chunk };
    const appliedFeatures: string[] = [];

    for (const d of sorted) {
        Object.assign(resolved, d.delta);
        appliedFeatures.push(d.featureId);
    }

    return { ...resolved, _appliedFeatures: appliedFeatures, _hasDeltas: true };
}

export function resolveChunks<T extends Record<string, unknown>>(
    chunks: T[],
    activeFeatureIds: string[],
    allDeltas: DeltaWithChunk[]
): (T & ResolvedMeta)[] {
    if (activeFeatureIds.length === 0 || allDeltas.length === 0) {
        return chunks.map(c => ({ ...c, _appliedFeatures: [] as string[], _hasDeltas: false }));
    }

    const deltasByChunk = new Map<string, Delta[]>();
    for (const d of allDeltas) {
        const existing = deltasByChunk.get(d.chunkId) ?? [];
        existing.push({ featureId: d.featureId, delta: d.delta, priority: d.priority });
        deltasByChunk.set(d.chunkId, existing);
    }

    return chunks.map(chunk => {
        const chunkId = (chunk as Record<string, unknown>).id as string;
        const deltas = deltasByChunk.get(chunkId) ?? [];
        return resolveChunk(chunk, deltas);
    });
}
