import type { Edge, Node } from "@xyflow/react";

export interface MermaidExportResult {
    text: string;
    nodeCount: number;
    edgeCount: number;
    truncated: boolean;
}

export interface MermaidExportOptions {
    maxNodes?: number;
    direction?: "LR" | "TB";
}

const DEFAULT_MAX_NODES = 100;

export function buildMermaidFromGraph(
    nodes: Node[],
    edges: Edge[],
    opts: MermaidExportOptions = {}
): MermaidExportResult {
    const maxNodes = opts.maxNodes ?? DEFAULT_MAX_NODES;
    const direction = opts.direction ?? "LR";

    const chunkNodes = nodes.filter(n => !n.id.startsWith("tag-group-") && !n.id.startsWith("group-"));
    const truncated = chunkNodes.length > maxNodes;
    const keptNodes = truncated ? chunkNodes.slice(0, maxNodes) : chunkNodes;
    const keptIds = new Set(keptNodes.map(n => n.id));
    const keptEdges = edges.filter(e => keptIds.has(e.source) && keptIds.has(e.target));

    const lines: string[] = [`flowchart ${direction}`];

    for (const node of keptNodes) {
        const data = node.data as { label?: string; type?: string } | undefined;
        const label = data?.label ?? node.id;
        lines.push(`    ${safeId(node.id)}["${escapeLabel(label)}"]`);
    }

    for (const edge of keptEdges) {
        const relation = (edge.data as { relation?: string } | undefined)?.relation ?? "related";
        lines.push(`    ${safeId(edge.source)} -->|${escapeLabel(relation)}| ${safeId(edge.target)}`);
    }

    return {
        text: lines.join("\n"),
        nodeCount: keptNodes.length,
        edgeCount: keptEdges.length,
        truncated
    };
}

function safeId(id: string): string {
    return "n" + id.replace(/[^a-zA-Z0-9]/g, "");
}

function escapeLabel(raw: string): string {
    return raw.replace(/"/g, "'").replace(/[[\]]/g, "").replace(/\|/g, "／");
}
