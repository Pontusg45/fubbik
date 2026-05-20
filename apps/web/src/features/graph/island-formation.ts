export interface IslandFormationInput {
    chunks: Array<{ id: string; type: string }>;
    connections: Array<{ sourceId: string; targetId: string; relation: string }>;
    chunkTags: Array<{ chunkId: string; tagTypeId: string | null; tagName: string }>;
    groupingTagTypeId: string;
}

export interface Island {
    id: string;
    name: string;
    chunkIds: string[];
    ghostChunkIds: string[];
    isSingleton: boolean;
}

export interface IslandBridge {
    fromIslandId: string;
    toIslandId: string;
    count: number;
    dominantRelation: string;
}

export interface IslandFormationResult {
    islands: Island[];
    bridges: IslandBridge[];
    chunkToIsland: Map<string, string>;
}

export function formIslands(input: IslandFormationInput): IslandFormationResult {
    const { chunks, connections, chunkTags, groupingTagTypeId } = input;

    const chunkTagNames = new Map<string, string[]>();
    for (const ct of chunkTags) {
        if (ct.tagTypeId !== groupingTagTypeId) continue;
        const existing = chunkTagNames.get(ct.chunkId);
        if (existing) existing.push(ct.tagName);
        else chunkTagNames.set(ct.chunkId, [ct.tagName]);
    }

    const islandChunks = new Map<string, string[]>();
    const islandGhosts = new Map<string, string[]>();
    const chunkToIsland = new Map<string, string>();

    for (const chunk of chunks) {
        const tags = chunkTagNames.get(chunk.id);
        if (!tags || tags.length === 0) {
            const arr = islandChunks.get("ungrouped") ?? [];
            arr.push(chunk.id);
            islandChunks.set("ungrouped", arr);
            chunkToIsland.set(chunk.id, "ungrouped");
            continue;
        }
        const primary = tags[0]!;
        const arr = islandChunks.get(primary) ?? [];
        arr.push(chunk.id);
        islandChunks.set(primary, arr);
        chunkToIsland.set(chunk.id, primary);

        for (let i = 1; i < tags.length; i++) {
            const ghostArr = islandGhosts.get(tags[i]!) ?? [];
            ghostArr.push(chunk.id);
            islandGhosts.set(tags[i]!, ghostArr);
        }
    }

    const islands: Island[] = [];
    for (const [name, chunkIds] of islandChunks) {
        islands.push({
            id: name,
            name,
            chunkIds,
            ghostChunkIds: islandGhosts.get(name) ?? [],
            isSingleton: chunkIds.length === 1 && name !== "ungrouped",
        });
    }
    for (const [name] of islandGhosts) {
        if (!islandChunks.has(name)) {
            islands.push({ id: name, name, chunkIds: [], ghostChunkIds: islandGhosts.get(name)!, isSingleton: false });
        }
    }

    const bridgeKey = (a: string, b: string) => a < b ? `${a}|${b}` : `${b}|${a}`;
    const bridgeData = new Map<string, { from: string; to: string; relations: string[] }>();

    for (const conn of connections) {
        const fromIsland = chunkToIsland.get(conn.sourceId);
        const toIsland = chunkToIsland.get(conn.targetId);
        if (!fromIsland || !toIsland || fromIsland === toIsland) continue;

        const key = bridgeKey(fromIsland, toIsland);
        const existing = bridgeData.get(key);
        if (existing) {
            existing.relations.push(conn.relation);
        } else {
            bridgeData.set(key, { from: fromIsland, to: toIsland, relations: [conn.relation] });
        }
    }

    const bridges: IslandBridge[] = [];
    for (const data of bridgeData.values()) {
        const freq = new Map<string, number>();
        for (const r of data.relations) freq.set(r, (freq.get(r) ?? 0) + 1);
        let dominant = data.relations[0]!;
        let maxCount = 0;
        for (const [r, c] of freq) {
            if (c > maxCount) { dominant = r; maxCount = c; }
        }
        bridges.push({
            fromIslandId: data.from,
            toIslandId: data.to,
            count: data.relations.length,
            dominantRelation: dominant,
        });
    }

    return { islands, bridges, chunkToIsland };
}
