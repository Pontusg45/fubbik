import { runForceLayout } from "./force-layout";

export interface LayoutWorkerInput {
    requestId: number;
    nodes: { id: string; type: string }[];
    edges: { source: string; target: string; relation: string }[];
    tagGroups?: Record<string, string[]>;
}

export interface LayoutWorkerOutput {
    requestId: number;
    positions: Record<string, { x: number; y: number }>;
}

self.onmessage = (e: MessageEvent<LayoutWorkerInput>) => {
    const { requestId, nodes, edges, tagGroups } = e.data;
    const tagGroupsMap = tagGroups ? new Map(Object.entries(tagGroups)) : undefined;
    const positions = runForceLayout(nodes, edges, tagGroupsMap);
    self.postMessage({ requestId, positions } satisfies LayoutWorkerOutput);
};
