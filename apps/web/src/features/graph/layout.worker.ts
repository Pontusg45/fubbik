import { runForceLayout } from "./force-layout";

export interface LayoutWorkerInput {
    requestId: number;
    nodes: { id: string; type: string }[];
    edges: { source: string; target: string; relation: string }[];
}

export interface LayoutWorkerOutput {
    requestId: number;
    positions: Record<string, { x: number; y: number }>;
}

self.onmessage = (e: MessageEvent<LayoutWorkerInput>) => {
    const { requestId, nodes, edges } = e.data;
    const positions = runForceLayout(nodes, edges);
    self.postMessage({ requestId, positions } satisfies LayoutWorkerOutput);
};
