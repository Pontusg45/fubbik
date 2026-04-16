import { fetchDensityPaths, type DensityPath } from "@fubbik/db/repository";
import { Effect } from "effect";

export interface DensityNode {
    name: string;
    path: string;
    chunkCount: number;
    directChunkCount: number;
    children: DensityNode[];
    chunks: Array<{ id: string; title: string; type: string; source: "applies_to" | "file_ref" }>;
}

export function getDensity(userId: string, codebaseId?: string) {
    return fetchDensityPaths(userId, codebaseId).pipe(
        Effect.map(paths => {
            const root = buildTree(paths);
            const totals = {
                chunksCovered: new Set(paths.map(p => p.chunkId)).size,
                pathsTracked: paths.length
            };
            return { tree: root, totals };
        })
    );
}

function buildTree(paths: DensityPath[]): DensityNode {
    const root: DensityNode = {
        name: "(root)",
        path: "",
        chunkCount: 0,
        directChunkCount: 0,
        children: [],
        chunks: []
    };

    const nodeMap = new Map<string, DensityNode>();
    nodeMap.set("", root);

    for (const p of paths) {
        const segments = p.path.split("/").filter(Boolean);
        let currentPath = "";
        let parent = root;

        for (const seg of segments) {
            currentPath = currentPath ? `${currentPath}/${seg}` : seg;
            const existing = nodeMap.get(currentPath);
            if (existing) {
                parent = existing;
                continue;
            }
            const node: DensityNode = {
                name: seg,
                path: currentPath,
                chunkCount: 0,
                directChunkCount: 0,
                children: [],
                chunks: []
            };
            nodeMap.set(currentPath, node);
            parent.children.push(node);
            parent = node;
        }

        const leaf = nodeMap.get(p.path);
        if (leaf && !leaf.chunks.some(c => c.id === p.chunkId && c.source === p.source)) {
            leaf.chunks.push({
                id: p.chunkId,
                title: p.chunkTitle,
                type: p.chunkType,
                source: p.source
            });
        }
    }

    computeCounts(root);
    return root;
}

function computeCounts(node: DensityNode): Set<string> {
    const childSeen = new Set<string>();
    for (const child of node.children) {
        const childSet = computeCounts(child);
        for (const id of childSet) childSeen.add(id);
    }
    node.directChunkCount = node.chunks.length;
    const all = new Set(childSeen);
    for (const c of node.chunks) all.add(c.id);
    node.chunkCount = all.size;
    node.children.sort((a, b) => b.chunkCount - a.chunkCount || a.name.localeCompare(b.name));
    return all;
}
